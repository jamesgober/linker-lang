//! Shared helpers for the linker integration tests.
//!
//! The bridge from a compiled `codegen-lang` program to a linkable [`Object`] lives here —
//! the seam where the language pipeline hands its output to the linker. A native backend
//! would emit real target machine code for a function's `.text`; for these tests a compact
//! one-byte-per-op encoding stands in, which is enough to give each section a deterministic
//! length and place the function's symbol at its start.

#![allow(
    dead_code,
    clippy::unwrap_used,
    reason = "test-support code: each test crate uses a subset, and a bad value should fail loudly"
)]

use codegen_lang::{Op, Program};
use linker_lang::Object;

/// Encodes a compiled program's op stream as one tag byte per op, so the section length
/// tracks the function's size. The exact bytes are a test stand-in for machine code.
#[must_use]
pub fn encode(program: &Program) -> Vec<u8> {
    program.ops().iter().map(opcode).collect()
}

/// A stable tag byte for each op kind.
fn opcode(op: &Op) -> u8 {
    match op {
        Op::Const { .. } => 0x01,
        Op::Bin { .. } => 0x02,
        Op::Un { .. } => 0x03,
        Op::Move { .. } => 0x04,
        Op::Jump { .. } => 0x05,
        Op::JumpUnless { .. } => 0x06,
        Op::Return { .. } => 0x07,
    }
}

/// Wraps a compiled program as a linkable object: its encoded ops become a `.text` section
/// and the function's name a symbol at offset `0`, where execution of the function begins.
#[must_use]
pub fn object_for(program: &Program) -> Object {
    let mut object = Object::new(program.name());
    object.section(".text", encode(program));
    object.define(program.name(), ".text", 0);
    object
}

/// Reads a little-endian `u64` at `at` — the inverse of a [`Width::U64`](linker_lang::Width)
/// relocation, for checking a patched slot.
#[must_use]
pub fn read_u64(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

/// Reads a little-endian `u32` at `at` — the inverse of a [`Width::U32`](linker_lang::Width)
/// relocation.
#[must_use]
pub fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
