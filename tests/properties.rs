//! Property tests: linking must preserve layout arithmetic and patch every relocation to
//! the address it resolved to.
//!
//! A random set of code objects (each a `.text` section of some size with a symbol at its
//! start) plus a random table of relocations into a `.data` section is generated, linked,
//! and checked against an independent computation of where everything should land — symbol
//! addresses against the running section offsets, and each patched slot against its
//! target's resolved address.

#![allow(
    clippy::unwrap_used,
    reason = "the generator only builds valid links, so they cannot fail"
)]

use linker_lang::{Object, Width, link};
use proptest::prelude::*;

/// `(text_sizes, table)` — one code object per entry in `text_sizes`, and one `.data` slot
/// per entry in `table`, each pointing at a (raw, later reduced) function index with an
/// addend.
fn inputs() -> impl Strategy<Value = (Vec<usize>, Vec<(usize, u8)>)> {
    (
        proptest::collection::vec(1usize..=64, 1..6),
        proptest::collection::vec((any::<usize>(), 0u8..=16), 0..6),
    )
}

/// Builds the objects: a `.text`-only object `f{i}` per size, then a loader whose `.data`
/// holds one `u64` slot per table entry, relocated to `f{index % n}` plus the addend.
fn build(text_sizes: &[usize], table: &[(usize, u8)]) -> Vec<Object> {
    let n = text_sizes.len();
    let mut objects = Vec::with_capacity(n + 1);
    for (i, &size) in text_sizes.iter().enumerate() {
        let mut object = Object::new(format!("f{i}"));
        object.section(".text", vec![0u8; size]);
        object.define(format!("f{i}"), ".text", 0);
        objects.push(object);
    }

    let mut loader = Object::new("loader");
    loader.section(".data", vec![0u8; table.len() * 8]);
    for (slot, &(raw, addend)) in table.iter().enumerate() {
        let target = raw % n;
        loader.relocate(
            ".data",
            (slot * 8) as u64,
            format!("f{target}"),
            Width::U64,
            i64::from(addend),
        );
    }
    objects.push(loader);
    objects
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}

proptest! {
    #[test]
    fn symbols_resolve_to_the_running_section_offset((sizes, table) in inputs()) {
        let image = link(&build(&sizes, &table)).unwrap();

        // `.text` is every function's code concatenated, so its length is the total.
        let total: usize = sizes.iter().sum();
        prop_assert_eq!(image.section(".text").unwrap().len(), total);

        // Symbol `f{i}` sits at the sum of the sizes before it.
        let mut offset = 0u64;
        for (i, &size) in sizes.iter().enumerate() {
            prop_assert_eq!(image.symbol(&format!("f{i}")), Some(offset));
            offset += size as u64;
        }
    }

    #[test]
    fn every_slot_is_patched_with_its_resolved_target((sizes, table) in inputs()) {
        let image = link(&build(&sizes, &table)).unwrap();
        let data = image.section(".data").unwrap().data();
        let n = sizes.len();

        for (slot, &(raw, addend)) in table.iter().enumerate() {
            let target = raw % n;
            let expected = image.symbol(&format!("f{target}")).unwrap() + u64::from(addend);
            prop_assert_eq!(read_u64(data, slot * 8), expected);
        }
    }

    #[test]
    fn linking_is_deterministic((sizes, table) in inputs()) {
        let first = link(&build(&sizes, &table)).unwrap();
        let second = link(&build(&sizes, &table)).unwrap();
        prop_assert_eq!(first, second);
    }
}
