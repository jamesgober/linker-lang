//! The linker must refuse a link it cannot complete, naming the precise reason.
//!
//! These build inputs the [`Object`] builder accepts but [`link`] rejects, and confirm the
//! returned [`LinkError`] is the specific variant — with the object, symbol, or section it
//! points at — rather than a silent miscompile or a partial image.

use linker_lang::{LinkError, Linker, Object, Width, link};

#[test]
fn test_a_symbol_defined_by_two_objects_is_rejected() {
    let mut a = Object::new("a.o");
    a.section(".text", [0u8; 4]);
    a.define("shared", ".text", 0);

    let mut b = Object::new("b.o");
    b.section(".text", [0u8; 4]);
    b.define("shared", ".text", 0);

    assert_eq!(
        link(&[a, b]),
        Err(LinkError::DuplicateSymbol {
            name: "shared".into()
        })
    );
}

#[test]
fn test_a_relocation_to_an_undefined_symbol_is_rejected() {
    let mut obj = Object::new("uses.o");
    obj.section(".data", [0u8; 8]);
    obj.relocate(".data", 0, "external", Width::U64, 0);

    assert_eq!(
        link(&[obj]),
        Err(LinkError::UndefinedSymbol {
            name: "external".into(),
            object: "uses.o".into(),
        })
    );
}

#[test]
fn test_an_undefined_entry_point_is_rejected() {
    let mut obj = Object::new("lib.o");
    obj.section(".text", [0u8; 4]);
    obj.define("helper", ".text", 0); // defines `helper`, but the entry is `_start`

    assert_eq!(
        Linker::new().entry("_start").link(&[obj]),
        Err(LinkError::UndefinedEntry {
            name: "_start".into()
        })
    );
}

#[test]
fn test_a_symbol_in_a_section_the_object_never_created_is_rejected() {
    let mut obj = Object::new("typo.o");
    obj.section(".text", [0u8; 4]);
    obj.define("misplaced", ".txet", 0); // `.txet`, not `.text`

    assert_eq!(
        link(&[obj]),
        Err(LinkError::InvalidSection {
            object: "typo.o".into(),
            section: ".txet".into(),
        })
    );
}

#[test]
fn test_a_relocation_running_past_its_section_is_rejected() {
    let mut obj = Object::new("short.o");
    obj.section(".text", [0u8; 1]);
    obj.define("t", ".text", 0);
    obj.section(".data", [0u8; 4]);
    obj.relocate(".data", 2, "t", Width::U32, 0); // 2 + 4 > 4

    assert_eq!(
        link(&[obj]),
        Err(LinkError::RelocationOutOfRange {
            object: "short.o".into(),
            section: ".data".into(),
            offset: 2,
        })
    );
}

#[test]
fn test_an_address_too_large_for_the_relocation_width_is_rejected() {
    let mut obj = Object::new("wide.o");
    obj.section(".text", [0u8; 4]);
    obj.define("t", ".text", 0);
    obj.section(".data", [0u8; 4]);
    obj.relocate(".data", 0, "t", Width::U32, 0);

    // A base address above 2^32 cannot fit in a 32-bit slot.
    assert_eq!(
        Linker::new().base_address(0x1_0000_0000).link(&[obj]),
        Err(LinkError::RelocationOverflow {
            target: "t".into(),
            width: Width::U32,
        })
    );
}

#[test]
fn test_link_error_is_usable_as_a_std_error() {
    let mut obj = Object::new("o");
    obj.section(".data", [0u8; 8]);
    obj.relocate(".data", 0, "missing", Width::U64, 0);

    let err = link(&[obj]).expect_err("the target is undefined");
    // Carries an actionable message and is a `std::error::Error`.
    let message = std::string::ToString::to_string(&err);
    assert!(message.contains("missing"));
    let _as_error: &dyn std::error::Error = &err;
}

#[test]
fn test_a_symbol_past_its_section_cannot_point_into_another_object() {
    // `a.o` contributes 4 bytes of `.text`; offset 16 would land inside `b.o`'s code.
    let mut a = Object::new("a.o");
    a.section(".text", [0xCC; 4]);
    a.define("hook", ".text", 16);
    let mut b = Object::new("b.o");
    b.section(".text", [0x90; 32]);
    b.define("b_fn", ".text", 0);

    assert_eq!(
        link(&[a, b]),
        Err(LinkError::SymbolOutOfRange {
            name: "hook".into(),
            object: "a.o".into(),
            section: ".text".into(),
            offset: 16,
        })
    );
}

#[test]
fn test_a_symbol_far_past_the_image_is_rejected() {
    let mut obj = Object::new("o");
    obj.section(".text", [0u8; 4]);
    obj.define("far", ".text", 1 << 40);

    assert!(matches!(
        Linker::new().base_address(0x1000).link(&[obj]),
        Err(LinkError::SymbolOutOfRange { offset, .. }) if offset == 1 << 40
    ));
}

#[test]
fn test_a_symbol_one_past_the_end_is_an_end_marker() {
    let mut a = Object::new("a.o");
    a.section(".text", [0u8; 4]);
    a.define("a_end", ".text", 4);
    let mut b = Object::new("b.o");
    b.section(".text", [0u8; 4]);

    let image = Linker::new().base_address(0x100).link(&[a, b]).unwrap();
    assert_eq!(image.symbol("a_end"), Some(0x104));
}

#[test]
fn test_an_entry_point_at_the_end_marker_is_rejected() {
    let mut a = Object::new("a.o");
    a.section(".text", [0u8; 4]);
    a.define("start", ".text", 4); // one past the end: valid symbol, not valid entry
    let mut b = Object::new("b.o");
    b.section(".text", [0u8; 4]);

    assert_eq!(
        Linker::new().entry("start").link(&[a, b]),
        Err(LinkError::EntryOutOfRange {
            name: "start".into()
        })
    );
}

#[test]
fn test_overlapping_relocations_are_rejected() {
    let mut obj = Object::new("o");
    obj.section(".data", [0u8; 12]);
    obj.define("x", ".data", 0);
    obj.relocate(".data", 0, "x", Width::U64, 0x1111);
    obj.relocate(".data", 4, "x", Width::U64, 0x2222);

    assert_eq!(
        link(&[obj]),
        Err(LinkError::OverlappingRelocations {
            object: "o".into(),
            section: ".data".into(),
            offset: 4,
        })
    );
}

#[test]
fn test_adjacent_relocations_are_accepted() {
    let mut obj = Object::new("o");
    obj.section(".data", [0u8; 12]);
    obj.define("x", ".data", 0);
    obj.relocate(".data", 8, "x", Width::U32, 1);
    obj.relocate(".data", 0, "x", Width::U64, 2);

    let image = link(&[obj]).unwrap();
    let data = image.section(".data").unwrap().data();
    assert_eq!(&data[..8], &2u64.to_le_bytes());
    assert_eq!(&data[8..], &1u32.to_le_bytes());
}
