//! Link a few objects with a cross-object relocation and print the resulting link map.
//!
//! This is the linker on its own: hand-built objects, no compiler in front of it. A code
//! object defines two functions; a data object holds a pointer slot that must point at one
//! of them. Linking lays them out, resolves the symbols, patches the slot, and the printed
//! [`Image`](linker_lang::Image) map shows where everything ended up.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example link_map
//! ```

use linker_lang::{Linker, Object, Width};

fn main() {
    // The code: two functions in `.text`, `main` at the top and `handler` after it.
    let mut code = Object::new("code.o");
    code.section(".text", vec![0u8; 12]);
    code.define("main", ".text", 0);
    code.define("handler", ".text", 6);

    // The data: an 8-byte slot that should hold the address of `handler` — a callback the
    // linker fills in once `handler` has an address.
    let mut data = Object::new("data.o");
    data.section(".data", vec![0u8; 8]);
    data.relocate(".data", 0, "handler", Width::U64, 0);

    let image = Linker::new()
        .base_address(0x40_0000)
        .entry("main")
        .link(&[code, data])
        .expect("every referenced symbol is defined");

    // The link map: entry point, each section's address and size, and the symbol table.
    print!("{image}");

    let slot = image.section(".data").unwrap().data();
    let patched = u64::from_le_bytes(slot.try_into().unwrap());
    println!("\n.data slot holds {patched:#x}, the address of `handler`");
    assert_eq!(patched, image.symbol("handler").unwrap());
}
