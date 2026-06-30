<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br>
    <b>linker-lang</b>
    <br>
    <sub><sup>OBJECT LINKER</sup></sub>
</h1>

<div align="center">
    <a href="https://crates.io/crates/linker-lang"><img alt="Crates.io" src="https://img.shields.io/crates/v/linker-lang"></a>
    <a href="https://crates.io/crates/linker-lang"><img alt="Downloads" src="https://img.shields.io/crates/d/linker-lang?color=%230099ff"></a>
    <a href="https://docs.rs/linker-lang"><img alt="docs.rs" src="https://img.shields.io/docsrs/linker-lang"></a>
    <a href="https://github.com/jamesgober/linker-lang/actions"><img alt="CI" src="https://github.com/jamesgober/linker-lang/actions/workflows/ci.yml/badge.svg"></a>
    <a href="https://github.com/rust-lang/rfcs/blob/master/text/2495-min-rust-version.md"><img alt="MSRV" src="https://img.shields.io/badge/MSRV-1.85%2B-blue"></a>
</div>

<br>

<div align="left">
    <p>
        linker-lang is the CODE-tier crate: it combines independently compiled objects into a single laid-out image — symbol resolution, section merging, and relocation patching. Part of the -lang language-construction family; see _strategy/LANG_COLLECTION.md for the master plan.
    </p>
    <br>
    <hr>
    <p>
        <strong>MSRV is 1.85+</strong> (Rust 2024 edition).
    </p>
    <blockquote>
        <strong>Status: pre-1.0, in active development.</strong> The public API is being designed across the 0.x series and frozen at <code>1.0.0</code>. See <a href="./docs/API.md"><code>docs/API.md</code></a> and the <a href="./CHANGELOG.md"><code>CHANGELOG.md</code></a>.
    </blockquote>
</div>

<hr>
<br>

## Overview

A compiler's back end emits each translation unit on its own: a few named byte sections, some symbols that mark addresses inside them, and relocations that still need an address filled in. Linking is the step that joins those islands into something you can load.

linker-lang does exactly that, and nothing more. Hand it a list of [`Object`](./docs/API.md#object)s and it lays their sections out end to end, resolves every [symbol](./docs/API.md#object) to its final address in that layout, and patches each [relocation](./docs/API.md#object) with the address it was waiting for. The result is an [`Image`](./docs/API.md#image) whose bytes are ready to load and whose symbol table says where everything ended up.

The model is format-agnostic. Nothing in it names ELF or PE: an `Object` is what a reader for either format would populate, and an `Image` is what a writer would serialize. The crate carries the linking logic without committing to a container format, so a reader and a writer slot in on either side of it later.

<br>
<hr>
<br>

## Installation

```toml
[dependencies]
linker-lang = "0.2"
```

Or from the terminal:

```bash
cargo add linker-lang
```

<br>

## Quick Start

```rust
use linker_lang::{link, Object, Width};

// The code: a symbol `start` at the top of a `.text` section.
let mut code = Object::new("code");
code.section(".text", [0x90, 0x90, 0x90, 0x90]);
code.define("start", ".text", 0);

// The data: an 8-byte slot that should hold the address of `start`.
let mut data = Object::new("data");
data.section(".data", [0u8; 8]);
data.relocate(".data", 0, "start", Width::U64, 0);

let image = link(&[code, data]).expect("symbols resolve");

// `.text` is laid out first at address 0, so `start` resolves there, and the slot in
// `.data` was patched with that address.
assert_eq!(image.symbol("start"), Some(0));
let slot = image.section(".data").unwrap().data();
assert_eq!(u64::from_le_bytes(slot.try_into().unwrap()), 0);

// The Display impl is a readable link map:
//   entry = none
//   .text @ 0x0000000000000000 (4 bytes)
//   .data @ 0x0000000000000004 (8 bytes)
//   symbols:
//       0x0000000000000000 start
println!("{image}");
```

<br>
<hr>
<br>

## Linking model

A link takes a list of objects and produces one image. Three passes do the work:

| Pass | What it does |
|---|---|
| **Merge sections** | Sections that share a name across objects are concatenated, in object order, into one output [section](./docs/API.md#outputsection). The output sections are placed at addresses, end to end from the base address. |
| **Resolve symbols** | Each [symbol](./docs/API.md#object) resolves to an absolute address — its section's address, plus where its object landed in that section, plus the offset. A name defined twice is a [`DuplicateSymbol`](./docs/API.md#linkerror). |
| **Patch relocations** | Each [relocation](./docs/API.md#object) is a hole that must hold the address of a symbol, plus an addend. The linker resolves the target and writes the address into the section bytes, little-endian, in the requested [`Width`](./docs/API.md#width). |

Sections are addressed by name, so building an object reads the way the layout does:

```rust
use linker_lang::{Linker, Object, Width};

// Two compiled functions go into `.text`; a dispatch table in `.data` points at them.
let mut code = Object::new("code");
code.section(".text", vec![0u8; 16]);
code.define("on_start", ".text", 0);
code.define("on_tick", ".text", 8);

let mut table = Object::new("table");
table.section(".data", vec![0u8; 16]);
table.relocate(".data", 0, "on_start", Width::U64, 0);
table.relocate(".data", 8, "on_tick", Width::U64, 0);

let image = Linker::new().base_address(0x40_0000).entry("on_start").link(&[code, table]).unwrap();

assert_eq!(image.entry(), image.symbol("on_start"));
let slots = image.section(".data").unwrap().data();
assert_eq!(u64::from_le_bytes(slots[0..8].try_into().unwrap()), image.symbol("on_start").unwrap());
assert_eq!(u64::from_le_bytes(slots[8..16].try_into().unwrap()), image.symbol("on_tick").unwrap());
```

<br>
<hr>
<br>

## API Overview

For a complete reference with examples, see [`docs/API.md`](./docs/API.md).

- [`link`](./docs/API.md#link) — the one-call shortcut: link with the default configuration.
- [`Linker`](./docs/API.md#linker) — the configurable form: a base address and an optional entry point.
- [`Object`](./docs/API.md#object) — a compilation unit: named byte sections, symbols, and relocations.
- [`Width`](./docs/API.md#width) — the size of a relocated address (`U32` / `U64`).
- [`Image`](./docs/API.md#image) — the linked result: laid-out sections, the symbol table, the entry point, and a link-map `Display`.
- [`OutputSection`](./docs/API.md#outputsection) — one laid-out section: name, address, and final bytes.
- [`LinkError`](./docs/API.md#linkerror) — the reason a link could not be completed.

### Error handling

Every way a link can fail is a distinct [`LinkError`](./docs/API.md#linkerror) variant that names the object, symbol, or section involved — a symbol defined twice, a relocation or entry point with no definition, a slot that runs past its section or an address that does not fit it. The linker reports the reason rather than producing a partial or wrong image.

```rust
use linker_lang::{link, LinkError, Object, Width};

let mut obj = Object::new("uses.o");
obj.section(".data", [0u8; 8]);
obj.relocate(".data", 0, "external", Width::U64, 0); // never defined

match link(&[obj]) {
    Err(LinkError::UndefinedSymbol { name, object }) => {
        assert_eq!((name.as_str(), object.as_str()), ("external", "uses.o"));
    }
    other => panic!("expected UndefinedSymbol, got {other:?}"),
}
```

<br>
<hr>
<br>

## Performance

Performance is a hard constraint, not an afterthought (see [`REPS.md`](./REPS.md)). A link is a few linear passes: sections are concatenated into preallocated buffers, symbol lookups go through an ordered map, and each relocation is one resolve and one little-endian write. The cost is linear in the total input size, with no copies beyond the section bytes themselves.

Run the benchmarks on your hardware for trends:

```bash
cargo bench --bench bench
```

The suite covers the two cost drivers — many small objects wired together by a relocation table (symbol resolution and patching dominate), and a few large sections (the byte merge dominates).

<br>
<hr>
<br>

## Configuration

### Feature Flags

| Feature | Default | Description |
|---|---|---|
| `std` | on | Links the standard library. Without it the crate is `#![no_std]` and needs only `alloc`. |
| `serde` | off | Derives `Serialize` / `Deserialize` for `Image`, `OutputSection`, `Width`, and `LinkError`, so a linked image or a failure can be cached, logged, or moved between tools. |

```toml
# no_std build:
linker-lang = { version = "0.2", default-features = false }

# with serialization:
linker-lang = { version = "0.2", features = ["serde"] }
```

<br>

## Examples

Two runnable examples live in [`examples/`](./examples):

```bash
# Link hand-built objects with a cross-object relocation and print the link map.
cargo run --example link_map

# Compile functions with `codegen-lang`, then link them into one image.
cargo run --example from_codegen
```

<br>

## Testing

```bash
# Unit, integration (workflow, invalid input, properties), and doc tests
cargo test --all-features

# Property tests only
cargo test --all-features --test properties

# Lints and formatting
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Benchmarks (Criterion)
cargo bench --bench bench
```

The integration suite in [`tests/workflow.rs`](./tests/workflow.rs) compiles functions with [`codegen-lang`](https://crates.io/crates/codegen-lang), wraps each as a linkable object, and links them — exercising symbol resolution, section merging, and relocation patching on real backend output. [`tests/invalid.rs`](./tests/invalid.rs) confirms every failure is reported as the precise error, and [`tests/properties.rs`](./tests/properties.rs) generates random links and checks symbol addresses and patched slots against an independent computation.

<br>

## Cross-Platform Support

The crate is pure, dependency-light Rust with no platform-specific code, and is tested on Linux, macOS, and Windows (x86_64) through the CI matrix on stable and the 1.85 MSRV.

<hr>
<br>

## Contributing

Engineering standards for this crate are the [Rust Efficiency &amp; Performance Standards](./REPS.md); the current scope and plan are in [`dev/ROADMAP.md`](./dev/ROADMAP.md). Before a PR: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features` must be clean.

<br>

<div id="license">
    <h2>License</h2>
    <p>Licensed under either of</p>
    <ul>
        <li><b>Apache License, Version 2.0</b> &mdash; <a href="./LICENSE-APACHE">LICENSE-APACHE</a></li>
        <li><b>MIT License</b> &mdash; <a href="./LICENSE-MIT">LICENSE-MIT</a></li>
    </ul>
    <p>at your option.</p>
</div>

<div align="center">
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2026 <strong>James Gober <me@jamesgober.com>.</strong></sup>
</div>
