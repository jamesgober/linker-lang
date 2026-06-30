<h1 align="center">
    <img width="99" alt="Rust logo" src="https://raw.githubusercontent.com/jamesgober/rust-collection/72baabd71f00e14aa9184efcb16fa3deddda3a0a/assets/rust-logo.svg">
    <br><b>linker-lang</b><br>
    <sub><sup>API REFERENCE</sup></sub>
</h1>
<div align="center">
    <sup>
        <a href="../README.md" title="Project Home"><b>HOME</b></a>
        <span>&nbsp;│&nbsp;</span>
        <span>API</span>
        <span>&nbsp;│&nbsp;</span>
        <a href="../dev/ROADMAP.md" title="Roadmap"><b>ROADMAP</b></a>
    </sup>
</div>
<br>

> **Status: pre-1.0.** The surface below is being designed across the 0.x series and will be frozen at `1.0.0`. [`LinkError`](#linkerror) is `#[non_exhaustive]`, so a new failure reason stays an additive change. See [`dev/ROADMAP.md`](../dev/ROADMAP.md).

A small static linker: it combines independently compiled [`Object`](#object)s into a single laid-out [`Image`](#image) — merging sections, resolving symbols, and patching relocations. The model is format-agnostic: an ELF or PE reader feeds the same `Object`, and a writer turns an `Image` back into a file.

<br>

## Table of Contents

- [Installation](#installation)
- [Linking model](#linking-model)
- [Public API](#public-api)
  - [`link`](#link)
  - [`Linker`](#linker)
  - [`Object`](#object)
  - [`Width`](#width)
  - [`Image`](#image)
  - [`OutputSection`](#outputsection)
  - [`LinkError`](#linkerror)
- [Feature flags](#feature-flags)
- [Stability](#stability)

<br>
<hr>
<br>

## Installation

```toml
[dependencies]
linker-lang = "0.2"
```

```bash
cargo add linker-lang
```

The crate is `#![no_std]`-capable: build with `default-features = false` to drop the standard library and depend only on `alloc`.

<br>
<hr>
<br>

## Linking model

A link takes a list of [`Object`](#object)s and produces one [`Image`](#image). Three things happen, in order:

- **Sections merge.** A [`section`](#object) is a named run of bytes (`.text`, `.data`, …). Sections that share a name across objects are concatenated into one output section, in the order the objects are given. The output sections are then placed at addresses, end to end from the linker's base address.
- **Symbols resolve.** A [`symbol`](#object) names an offset inside a section. Once the layout is fixed, each symbol resolves to an absolute address — its section's address, plus where its object's contribution landed in that section, plus the offset. A name defined by two objects is a [`DuplicateSymbol`](#linkerror).
- **Relocations patch.** A [`relocation`](#object) is a hole at some offset in a section that must hold the address of a named symbol, plus an addend. The linker looks the target up, adds the addend, and writes the result into the section bytes little-endian, in the requested [`Width`](#width).

The result is an [`Image`](#image) whose section bytes are final and whose symbol table records where everything ended up. Nothing in the model names a container format: the same `Object` is what an ELF or PE reader would populate, and an `Image` is what a writer would serialize.

<br>
<hr>
<br>

## Public API

### `link`

```rust
pub fn link(objects: &[Object]) -> Result<Image, LinkError>
```

Links `objects` into an [`Image`](#image) with the default configuration — base address `0`, no entry point. This is the shortcut for the common case; reach for [`Linker`](#linker) when you need to set the base address or an entry point.

**Parameters**

- `objects` — the compilation units to link. They are read, not modified. Their sections merge by name in this order, so the order is the layout order.

**Returns**

- `Ok(Image)` — the linked image.
- `Err(LinkError)` — see [`LinkError`](#linkerror) for the reasons a link can fail.

**Examples**

Merge two objects and read back the resolved symbol table:

```rust
use linker_lang::{link, Object};

let mut a = Object::new("a");
a.section(".text", [1, 2, 3, 4]);
a.define("a_start", ".text", 0);

let mut b = Object::new("b");
b.section(".text", [5, 6]);
b.define("b_start", ".text", 0);

let image = link(&[a, b]).unwrap();

// `.text` is the two contributions in object order; `b_start` follows a's four bytes.
assert_eq!(image.section(".text").unwrap().data(), &[1, 2, 3, 4, 5, 6]);
assert_eq!(image.symbol("a_start"), Some(0));
assert_eq!(image.symbol("b_start"), Some(4));
```

Patch a cross-object relocation:

```rust
use linker_lang::{link, Object, Width};

let mut code = Object::new("code");
code.section(".text", [0u8; 8]);
code.define("handler", ".text", 0);

let mut table = Object::new("table");
table.section(".data", [0u8; 8]);
table.relocate(".data", 0, "handler", Width::U64, 0);

let image = link(&[code, table]).unwrap();
let slot = image.section(".data").unwrap().data();
assert_eq!(u64::from_le_bytes(slot.try_into().unwrap()), 0); // handler's address
```

<br>

### `Linker`

```rust
pub struct Linker { /* private fields */ }
```

The configurable form of [`link`](#link): a base address and an optional entry point. Set them with the builder methods, then call [`Linker::link`](#linker). A linker holds no per-link state, so one instance links many sets of objects. Implements `Default` (base address `0`, no entry point).

**Methods**

| Method | Returns | Description |
|---|---|---|
| `new()` | `Linker` | A linker with the default configuration. Same as `Linker::default()`. |
| `base_address(addr)` | `Linker` | Sets the address the first output section is placed at; later sections follow. Every resolved address is relative to this base. |
| `entry(symbol)` | `Linker` | Sets the symbol whose address becomes the image's [`entry`](#image). The symbol must be defined, or the link fails with [`UndefinedEntry`](#linkerror). |
| `link(objects)` | `Result<Image, LinkError>` | Links `objects` with this configuration. See [`link`](#link) for the failure modes. |

**Examples**

Lay an image out at a load address with an entry point:

```rust
use linker_lang::{Linker, Object};

let mut obj = Object::new("o");
obj.section(".text", [0u8; 16]);
obj.define("_start", ".text", 0);

let image = Linker::new()
    .base_address(0x40_0000)
    .entry("_start")
    .link(&[obj])
    .expect("the entry point is defined");

assert_eq!(image.symbol("_start"), Some(0x40_0000));
assert_eq!(image.entry(), Some(0x40_0000));
```

Reuse one linker for several links:

```rust
use linker_lang::{Linker, Object};

let linker = Linker::new().base_address(0x1000);
for name in ["x", "y"] {
    let mut obj = Object::new(name);
    obj.section(".text", [0u8; 4]);
    obj.define(name, ".text", 0);
    assert_eq!(linker.link(&[obj]).unwrap().symbol(name), Some(0x1000));
}
```

<br>

### `Object`

```rust
pub struct Object { /* private fields */ }
```

One compilation unit handed to the linker: named byte sections, the symbols defined in them, and the relocations still to be patched. This is the linker's input island — the form a backend or an object-file reader produces. An object carries no addresses of its own; [`link`](#link) assigns them.

Sections are addressed by name. Appending to a name that already exists extends that section, so an object holds at most one section per name.

**Methods**

| Method | Returns | Description |
|---|---|---|
| `new(name)` | `Object` | An empty object. `name` identifies it in diagnostics. |
| `name()` | `&str` | The object's name. |
| `section(name, data)` | `u64` | Appends `data` to the section `name`, creating it if new. Returns the offset within the section where `data` begins — `0` for a fresh section, the previous length when extending. |
| `define(name, section, offset)` | `()` | Defines a symbol `name` at `offset` bytes into `section`. |
| `relocate(section, offset, target, width, addend)` | `()` | Records that the `width` bytes at `offset` in `section` hold the address of `target`, plus `addend`. |

**Parameters — `relocate`**

- `section` — the section the slot lives in.
- `offset` — the slot's byte offset within this object's contribution to that section.
- `target` — the symbol whose address fills the slot.
- `width` — the slot size: [`Width::U32`](#width) or [`Width::U64`](#width). The address is written little-endian.
- `addend` — a signed value added to the target's address before it is written.

**Examples**

Build an object with code and a symbol at its start:

```rust
use linker_lang::Object;

let mut obj = Object::new("greeter.o");
obj.section(".text", b"\xc3".to_vec()); // one byte standing in for `ret`
obj.define("greet", ".text", 0);
assert_eq!(obj.name(), "greeter.o");
```

Place several symbols in one section, using the returned offsets as anchors:

```rust
use linker_lang::Object;

let mut obj = Object::new("multi.o");
let first = obj.section(".text", [0u8; 8]);   // 0
obj.define("first", ".text", first);
let second = obj.section(".text", [0u8; 8]);  // 8 — appended after the first chunk
obj.define("second", ".text", second);
```

Build a function-pointer table that needs two relocations:

```rust
use linker_lang::{Object, Width};

let mut obj = Object::new("vectors.o");
obj.section(".data", [0u8; 16]);
obj.relocate(".data", 0, "entry", Width::U64, 0); // slot 0 -> address of `entry`
obj.relocate(".data", 8, "entry", Width::U64, 8); // slot 1 -> eight bytes past it
```

<br>

### `Width`

```rust
pub enum Width {
    U32,
    U64,
}
```

The size of the address a [relocation](#object) patches in. A relocation writes its resolved address as a little-endian integer of this width; the link fails with [`RelocationOverflow`](#linkerror) if the address does not fit.

**Variants**

| Variant | Bytes | Use for |
|---|---|---|
| `U32` | 4 | A 32-bit pointer slot. |
| `U64` | 8 | A 64-bit pointer slot. |

**Methods**

- `bytes() -> usize` — the number of bytes the width occupies (`4` or `8`).

**Examples**

```rust
use linker_lang::Width;

assert_eq!(Width::U32.bytes(), 4);
assert_eq!(Width::U64.bytes(), 8);
```

<br>

### `Image`

```rust
pub struct Image { /* private fields */ }
```

A linked image: the laid-out sections, the resolved symbol table, and the entry point. This is what [`link`](#link) produces on success. Its sections are placed at fixed addresses with their relocations patched, its symbol table maps every defined name to its address, and its entry is the address of the configured entry point, if any.

The `Display` implementation renders a readable link map: the entry point, each section with its address and size, and the symbol table sorted by name.

**Methods**

| Method | Returns | Description |
|---|---|---|
| `sections()` | `&[OutputSection]` | The laid-out sections, in address order. |
| `section(name)` | `Option<&OutputSection>` | The section named `name`, or `None`. |
| `symbol(name)` | `Option<u64>` | The resolved address of `name`, or `None` if no object defined it. |
| `symbols()` | `impl Iterator<Item = (&str, u64)>` | The symbol table as `(name, address)` pairs, sorted by name. |
| `entry()` | `Option<u64>` | The entry-point address, or `None` if none was configured. |
| `len()` | `usize` | The number of sections. |
| `is_empty()` | `bool` | Whether the image has no sections. |

**Examples**

Look up addresses and the entry point:

```rust
use linker_lang::{Linker, Object};

let mut obj = Object::new("o");
obj.section(".text", [0u8; 8]);
obj.define("main", ".text", 0);
obj.define("loop", ".text", 4);

let image = Linker::new().entry("main").link(&[obj]).unwrap();

assert_eq!(image.symbol("main"), Some(0));
assert_eq!(image.symbol("loop"), Some(4));
assert_eq!(image.entry(), Some(0));
assert_eq!(image.symbols().count(), 2);
```

Render the link map:

```rust
use linker_lang::{Linker, Object};

let mut obj = Object::new("o");
obj.section(".text", [0u8; 4]);
obj.define("main", ".text", 0);
let image = Linker::new().entry("main").link(&[obj]).unwrap();

let map = image.to_string();
assert!(map.contains("entry = 0x0000000000000000"));
assert!(map.contains(".text @ 0x0000000000000000 (4 bytes)"));
assert!(map.contains("0x0000000000000000 main"));
```

<br>

### `OutputSection`

```rust
pub struct OutputSection { /* private fields */ }
```

One laid-out section in an [`Image`](#image): the concatenation of every input section that shared its name, placed at a fixed address. Its bytes are final — relocations into it have been patched — so it is ready to be written to a file or loaded at its address.

**Methods**

| Method | Returns | Description |
|---|---|---|
| `name()` | `&str` | The section's name. |
| `address()` | `u64` | The address the section begins at. |
| `data()` | `&[u8]` | The section's final, relocated bytes. |
| `len()` | `usize` | The number of bytes. |
| `is_empty()` | `bool` | Whether the section has no bytes. |

**Examples**

```rust
use linker_lang::{link, Object};

let mut obj = Object::new("o");
obj.section(".text", [1, 2, 3, 4]);
let image = link(&[obj]).unwrap();

let text = image.section(".text").unwrap();
assert_eq!(text.name(), ".text");
assert_eq!(text.address(), 0);
assert_eq!(text.data(), &[1, 2, 3, 4]);
assert_eq!(text.len(), 4);
```

<br>

### `LinkError`

```rust
#[non_exhaustive]
pub enum LinkError {
    DuplicateSymbol { name: String },
    UndefinedSymbol { name: String, object: String },
    UndefinedEntry { name: String },
    InvalidSection { object: String, section: String },
    RelocationOutOfRange { object: String, section: String, offset: u64 },
    RelocationOverflow { target: String, width: Width },
    LayoutOverflow,
}
```

The reason a link could not be completed. Each variant points at the object, symbol, or section involved so the producer can be fixed. `LinkError` implements `Display` and `std::error::Error`. The enum is `#[non_exhaustive]`, so a `match` on it must include a wildcard arm.

**Variants**

| Variant | Meaning | What to do |
|---|---|---|
| `DuplicateSymbol { name }` | Two objects define `name`. | Rename one, or drop the duplicate definition. |
| `UndefinedSymbol { name, object }` | A relocation in `object` targets `name`, which nobody defines. | Define the symbol, or remove the reference. |
| `UndefinedEntry { name }` | The configured [entry point](#linker) is not defined. | Define it, or link without an entry point. |
| `InvalidSection { object, section }` | A symbol or relocation names a `section` the object never created. | Fix the section name — it is almost certainly a typo. |
| `RelocationOutOfRange { object, section, offset }` | A relocation's slot runs past the end of its section. | Correct the offset, or size the section to cover the slot. |
| `RelocationOverflow { target, width }` | The resolved address is negative after the addend, or larger than `width` holds. | Widen the slot, or adjust the layout so the address fits. |
| `LayoutOverflow` | The laid-out image would exceed the 64-bit address space. | Lower the base address or the total section size. |

**Examples**

Match on the reason:

```rust
use linker_lang::{link, LinkError, Object};

let mut a = Object::new("a");
a.section(".text", [0u8; 1]);
a.define("main", ".text", 0);

let mut b = Object::new("b");
b.section(".text", [0u8; 1]);
b.define("main", ".text", 0);

match link(&[a, b]) {
    Err(LinkError::DuplicateSymbol { name }) => assert_eq!(name, "main"),
    other => panic!("expected DuplicateSymbol, got {other:?}"),
}
```

Use it as a `std::error::Error`:

```rust
use linker_lang::{link, Object, Width};
use std::error::Error;

let mut obj = Object::new("uses.o");
obj.section(".data", [0u8; 8]);
obj.relocate(".data", 0, "external", Width::U64, 0);

let err = link(&[obj]).unwrap_err();
let _as_error: &dyn Error = &err;
assert!(err.to_string().contains("external"));
```

<br>
<hr>
<br>

## Feature flags

| Feature | Default | Description |
|---|---|---|
| `std` | on | Links the standard library. Without it the crate is `#![no_std]` and needs only `alloc`. |
| `serde` | off | Derives `Serialize` / `Deserialize` for [`Image`](#image), [`OutputSection`](#outputsection), [`Width`](#width), and [`LinkError`](#linkerror), so a linked image or a failure can be cached, logged, or moved between tools. |

```toml
# no_std build:
linker-lang = { version = "0.2", default-features = false }

# with serialization:
linker-lang = { version = "0.2", features = ["serde"] }
```

<br>

## Stability

The public surface is pre-1.0 and still being designed; it will be frozen at `1.0.0`, after which the crate follows [Semantic Versioning](https://semver.org) with no breaking changes before `2.0`. [`LinkError`](#linkerror) is `#[non_exhaustive]` so that a new failure reason — a new relocation kind, a new layout constraint — is an additive change rather than a breaking one. This file is updated in lockstep with every release so it always matches the code.

<br>
<hr>

<sub>Copyright &copy; 2026 <strong>James Gober</strong>.</sub>
