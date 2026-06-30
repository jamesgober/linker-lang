<h1 align="center">
    <img width="90px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br><b>CHANGELOG</b>
</h1>
<p>
  All notable changes to <code>linker-lang</code> will be documented in this file. The format is based on <a href="https://keepachangelog.com/en/1.1.0/">Keep a Changelog</a>,
  and this project adheres to <a href="https://semver.org/spec/v2.0.0.html/">Semantic Versioning</a>.
</p>

---

## [Unreleased]

### Added

### Changed

### Fixed

### Security

---

## [1.0.0] - 2026-06-30

API freeze. The public surface built in `0.2.0` — the `Object` model, the `Linker` and the `link` shortcut, the `Image`, and `LinkError` — is ratified as the `1.0` contract: it follows Semantic Versioning and carries no breaking changes before `2.0`. There is no behavioural change from `0.2.0`; a set of objects that linked then links identically now. The release adds the stability documentation and the one fix below.

### Changed

- Marked the public API stable and frozen as of `1.0.0`; recorded the SemVer promise in [`docs/API.md`](docs/API.md#semver-promise) and the crate root.

### Fixed

- Elided a redundant lifetime in `link.rs` that the MSRV (`1.85`) `clippy` flagged as `needless_lifetimes` — newer `clippy` does not, so it only surfaced on the 1.85 CI jobs. No API or behavioural change.

---

## [0.2.0] - 2026-06-30

The core milestone: the linker lands. This is the first release with domain logic — it combines independently compiled objects into a single laid-out image, merging sections, resolving symbols, and patching relocations. The public surface is documented in [`docs/API.md`](docs/API.md) and remains pre-1.0 (subject to change until the `1.0.0` freeze).

### Added

- `Object` — a compilation unit: named byte sections, the symbols defined in them, and the relocations to patch. Built with `new`, `section`, `define`, and `relocate`; sections are addressed by name and merge across objects.
- `Width` — the size of a relocated address (`U32` / `U64`), written little-endian.
- `Linker` — the configurable linker: a base address and an optional entry point, with `new`, `base_address`, `entry`, and `link`.
- `link` — the shortcut for linking with the default configuration.
- `Image` — the linked result: laid-out sections, the resolved symbol table, the entry point, accessors (`sections`, `section`, `symbol`, `symbols`, `entry`, `len`, `is_empty`), and a link-map `Display`.
- `OutputSection` — one laid-out section: name, address, and final relocated bytes.
- `LinkError` — the failure reason, `#[non_exhaustive]`, covering duplicate symbols, undefined symbols and entry points, invalid section names, out-of-range relocations, width overflow, and layout overflow; with `Display` and `Error` impls.
- `serde` derives for `Image`, `OutputSection`, `Width`, and `LinkError` behind the `serde` feature.
- Integration tests covering the end-to-end workflow — compiling functions with `codegen-lang` and linking them — invalid-input rejection tests, property tests checking the layout against an independent computation, and a `serde` round-trip suite.
- Two runnable examples (`link_map`, `from_codegen`) and Criterion benchmarks for the symbol-resolution and section-merge cost drivers.
- Full rustdoc with runnable examples on every public item; `docs/API.md` API reference.

### Changed

- Wired `codegen-lang` and `ir-lang` as dev-dependencies, used by the end-to-end workflow tests and the `from_codegen` example to drive a real IR → codegen → link pipeline. The library's public surface stays format-agnostic.

### Fixed

- Corrected the unparseable `keywords` and `categories` arrays in `Cargo.toml` that prevented the crate from building.
- Aligned `clippy.toml`'s `msrv` with the crate's declared `rust-version` (`1.85`).

---

## [0.1.0] - 2026-06-18

Initial scaffold and repository bootstrap. No domain logic yet &mdash; this release establishes the structure, tooling, and quality gates the implementation will be built on.

### Added

- `Cargo.toml` with crate metadata, Rust 2024 edition, MSRV 1.85.
- Dual `Apache-2.0 OR MIT` license files.
- `README.md`, `CHANGELOG.md`, and a documentation skeleton.
- `REPS.md` compliance baseline.
- `.github/workflows/ci.yml` CI matrix; `deny.toml`, `clippy.toml`, `rustfmt.toml`.
- `dev/ROADMAP.md` (committed plan).

[Unreleased]: https://github.com/jamesgober/linker-lang/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/jamesgober/linker-lang/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/jamesgober/linker-lang/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/jamesgober/linker-lang/releases/tag/v0.1.0
