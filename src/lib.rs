//! # linker_lang
//!
//! A small static linker: it combines independently compiled [`Object`]s into a single
//! laid-out [`Image`].
//!
//! Each compilation unit a backend produces is its own island — a few named byte
//! [sections](Object::section), a set of [symbols](Object::define) that mark addresses
//! inside them, and [relocations](Object::relocate) that still need an address filled in.
//! Linking is the step that joins those islands: it lays the sections of every object end
//! to end, resolves each symbol to its final address in that layout, and patches the
//! relocations with the addresses they were waiting for. The result is an [`Image`] whose
//! bytes are ready to load and whose symbol table says where everything ended up.
//!
//! [`link`] is the one-call entry point; [`Linker`] is the same thing with a base address
//! and an entry point to configure. The model is format-agnostic — an ELF or PE reader
//! feeds the same [`Object`], and a writer turns an [`Image`] back into a file — so the
//! crate carries the linking logic without committing to one container format.
//!
//! ## Linking model
//!
//! - A [`section`](Object::section) is a named run of bytes (`.text`, `.data`, …).
//!   Sections that share a name across objects are concatenated into one output section,
//!   in the order the objects are given.
//! - A [`symbol`](Object::define) names an offset inside a section. After layout it
//!   resolves to an absolute address — its section's address plus the object's place in
//!   that section plus the offset.
//! - A [`relocation`](Object::relocate) is a hole at some offset in a section that must
//!   hold the address of a named symbol (plus an addend). The linker resolves the target
//!   and writes the address into the section bytes, little-endian, in the requested
//!   [`Width`].
//!
//! ## Example
//!
//! Link two objects where a `.data` slot must point at a symbol defined in `.text`:
//!
//! ```
//! use linker_lang::{link, Width};
//!
//! // The code: a symbol `start` at the top of `.text`.
//! let mut code = linker_lang::Object::new("code");
//! code.section(".text", [0x90, 0x90, 0x90, 0x90]);
//! code.define("start", ".text", 0);
//!
//! // The data: an 8-byte slot that should hold the address of `start`.
//! let mut data = linker_lang::Object::new("data");
//! data.section(".data", [0; 8]);
//! data.relocate(".data", 0, "start", Width::U64, 0);
//!
//! let image = link(&[code, data]).expect("symbols resolve");
//!
//! // `.text` is laid out first at address 0, so `start` resolves there, and the slot in
//! // `.data` was patched with that address.
//! assert_eq!(image.symbol("start"), Some(0));
//! let slot = &image.section(".data").unwrap().data()[..8];
//! assert_eq!(u64::from_le_bytes(slot.try_into().unwrap()), 0);
//! ```
//!
//! ## Features
//!
//! - `std` (default) — links the standard library. Without it the crate is `#![no_std]`
//!   and needs only `alloc`.
//! - `serde` — derives `Serialize` and `Deserialize` for [`Image`] and [`LinkError`], so a
//!   linked image or a failure can be cached, logged, or moved between tools.
//!
//! ## Stability
//!
//! The public surface is being designed across the 0.x series and frozen at `1.0.0`.
//! [`LinkError`] is `#[non_exhaustive]`, so a linker reporting a new kind of failure stays
//! an additive change. See [`docs/API.md`](https://github.com/jamesgober/linker-lang/blob/main/docs/API.md)
//! and [`dev/ROADMAP.md`](https://github.com/jamesgober/linker-lang/blob/main/dev/ROADMAP.md).

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::dbg_macro,
    clippy::print_stdout,
    clippy::print_stderr
)]

extern crate alloc;

mod error;
mod image;
mod link;
mod object;

pub use error::LinkError;
pub use image::{Image, OutputSection};
pub use link::{Linker, link};
pub use object::{Object, Width};
