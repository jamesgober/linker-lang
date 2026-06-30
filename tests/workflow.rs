//! End-to-end linker workflow tests.
//!
//! Each test walks the path a front-end takes once it has code in hand: compile functions
//! with [`codegen_lang::compile`], wrap each compiled program as a linkable object, then
//! link them into a single image and check the layout, the resolved symbol table, and the
//! relocations the linker patched. This is the seam where the language pipeline meets the
//! linker, exercised on real backend output.

mod support;

use codegen_lang::{Program, compile};
use ir_lang::{BinOp, Builder, Type};
use linker_lang::{Linker, Width, link};
use support::{encode, object_for, read_u64};

/// `fn double(x: int) -> int { x + x }`
fn double() -> Program {
    let mut b = Builder::new("double", &[Type::Int], Type::Int);
    let x = b.block_params(b.entry())[0];
    let sum = b.bin(BinOp::Add, x, x);
    b.ret(Some(sum));
    compile(&b.finish()).expect("double is well-formed")
}

/// `fn square(x: int) -> int { x * x }`
fn square() -> Program {
    let mut b = Builder::new("square", &[Type::Int], Type::Int);
    let x = b.block_params(b.entry())[0];
    let product = b.bin(BinOp::Mul, x, x);
    b.ret(Some(product));
    compile(&b.finish()).expect("square is well-formed")
}

/// `fn main() {}`
fn main_fn() -> Program {
    let mut b = Builder::new("main", &[], Type::Unit);
    b.ret(None);
    compile(&b.finish()).expect("main is well-formed")
}

#[test]
fn test_compiled_functions_merge_into_one_text_section() {
    let (d, s, m) = (double(), square(), main_fn());
    let image = link(&[object_for(&d), object_for(&s), object_for(&m)])
        .expect("three distinct functions link cleanly");

    // The output `.text` is the three functions' code, concatenated in object order.
    let mut expected = encode(&d);
    expected.extend(encode(&s));
    expected.extend(encode(&m));
    assert_eq!(image.section(".text").unwrap().data(), expected.as_slice());

    // Each function's symbol resolves to where its code begins in that merged section.
    assert_eq!(image.symbol("double"), Some(0));
    assert_eq!(image.symbol("square"), Some(encode(&d).len() as u64));
    assert_eq!(
        image.symbol("main"),
        Some((encode(&d).len() + encode(&s).len()) as u64)
    );
}

#[test]
fn test_entry_point_resolves_to_the_main_function() {
    let (d, m) = (double(), main_fn());
    // `main` is given second, so it does not sit at address 0 — the entry must follow the
    // symbol, not assume a position.
    let image = Linker::new()
        .entry("main")
        .link(&[object_for(&d), object_for(&m)])
        .expect("main is defined");

    assert_eq!(image.entry(), image.symbol("main"));
    assert_eq!(image.entry(), Some(encode(&d).len() as u64));
}

#[test]
fn test_a_dispatch_table_relocates_to_compiled_function_addresses() {
    let (d, s, m) = (double(), square(), main_fn());

    // A loader object: a `.data` table of three 8-byte slots that must hold the addresses
    // of `main`, `double`, and `square` — a function-pointer table the linker fills in.
    let mut loader = linker_lang::Object::new("loader");
    loader.section(".data", [0u8; 24]);
    loader.relocate(".data", 0, "main", Width::U64, 0);
    loader.relocate(".data", 8, "double", Width::U64, 0);
    loader.relocate(".data", 16, "square", Width::U64, 0);

    let image = Linker::new()
        .base_address(0x40_0000)
        .entry("main")
        .link(&[object_for(&d), object_for(&s), object_for(&m), loader])
        .expect("every referenced symbol is defined");

    let table = image.section(".data").unwrap().data();
    assert_eq!(read_u64(table, 0), image.symbol("main").unwrap());
    assert_eq!(read_u64(table, 8), image.symbol("double").unwrap());
    assert_eq!(read_u64(table, 16), image.symbol("square").unwrap());

    // The table lives in `.data`, which is laid out after `.text`, above the base address.
    assert!(image.section(".data").unwrap().address() >= 0x40_0000);
}

#[test]
fn test_relocation_with_an_addend_points_into_a_function_body() {
    let d = double();
    let code_len = encode(&d).len() as u64;

    // A slot that should hold the address of `double`'s last op, not its entry.
    let mut loader = linker_lang::Object::new("loader");
    loader.section(".data", [0u8; 8]);
    loader.relocate(".data", 0, "double", Width::U64, (code_len - 1) as i64);

    let image = link(&[object_for(&d), loader]).expect("double is defined");
    let slot = image.section(".data").unwrap().data();
    assert_eq!(
        read_u64(slot, 0),
        image.symbol("double").unwrap() + code_len - 1
    );
}
