# linker-lang - Roadmap

> Path from scaffold to a stable 1.0. Hard parts are front-loaded; each phase has hard exit criteria.
> Master plan: ../../_strategy/LANG_COLLECTION.md
>
> **Anti-deferral rule:** no listed hard task moves to a later phase unless this file records the move and the reason.

## v0.1.0 - Scaffold (DONE)
Compiles, CI green, structure correct, no domain logic.
- [x] Manifest, README, CHANGELOG, REPS, dual license, CI, deny, clippy, rustfmt.

## v0.2.0 - Core (THE HARD PART, NOT DEFERRED) (DONE)
Symbol resolution, section merging, relocation patching, and output image generation, over
a format-agnostic `Object`/`Image` model. `codegen-lang` (and `ir-lang`) are wired as
dev-dependencies, used first by the end-to-end workflow tests and the `from_codegen`
example, which keeps the public surface free of any one container format or backend.
Exit criteria:
- [x] Every public item has rustdoc + a runnable example.
- [x] Core invariants property-tested (full API authored at this stage).

## v1.0.0 - API freeze (DONE)
Public surface stable and frozen until 2.0. No breaking change from 0.2.0.
- [x] docs/API.md marked stable; SemVer promise recorded (and in the crate root).
- [x] Full test + benchmark suite green on all three platforms (CI matrix, stable + 1.85 MSRV).
