//! Compile functions with `codegen-lang`, then link them into one image.
//!
//! This is the end of the language pipeline: a front-end builds IR, `codegen-lang` lowers
//! each function to code, and the linker joins those compiled units into a single image
//! with one symbol table. Here each compiled function's op stream stands in for its machine
//! code; a native backend would supply real bytes, and the linker would work the same way.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example from_codegen
//! ```

use codegen_lang::{Program, compile};
use ir_lang::{BinOp, Builder, Type};
use linker_lang::{Linker, Object};

fn main() {
    let functions = [double(), square(), main_fn()];

    // Wrap each compiled function as an object: its code becomes a `.text` section and its
    // name a symbol at the function's entry.
    let objects: Vec<Object> = functions.iter().map(object_for).collect();

    let image = Linker::new()
        .base_address(0x40_0000)
        .entry("main")
        .link(&objects)
        .expect("the three functions have distinct names");

    println!("linked {} function(s) into one image:\n", functions.len());
    print!("{image}");

    println!("\nentry point `main` is at {:#x}", image.entry().unwrap());
}

/// Wraps a compiled program as a linkable object — one `.text` section of encoded ops, with
/// the function name a symbol at offset 0.
fn object_for(program: &Program) -> Object {
    let code: Vec<u8> = program.ops().iter().map(|_| 0u8).collect();
    let mut object = Object::new(program.name());
    object.section(".text", code);
    object.define(program.name(), ".text", 0);
    object
}

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
