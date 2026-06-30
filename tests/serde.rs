//! Round-trip tests for the `serde` feature: a linked image and a link error survive
//! serialization and deserialization unchanged.

#![cfg(feature = "serde")]

use linker_lang::{Image, LinkError, Linker, Object, Width, link};

#[test]
fn test_image_round_trips_through_json() {
    // A link with two sections, several symbols, a patched relocation, and an entry point,
    // so the round trip covers every field of the image.
    let mut code = Object::new("code");
    code.section(".text", [0u8; 8]);
    code.define("main", ".text", 0);
    code.define("helper", ".text", 4);

    let mut data = Object::new("data");
    data.section(".data", [0u8; 8]);
    data.relocate(".data", 0, "helper", Width::U64, 0);

    let original = Linker::new()
        .base_address(0x1000)
        .entry("main")
        .link(&[code, data])
        .expect("symbols resolve");

    let json = serde_json::to_string(&original).expect("serialization succeeds");
    let restored: Image = serde_json::from_str(&json).expect("deserialization succeeds");

    assert_eq!(restored, original);
    // The link map renders identically, exercising the deserialized sections and symbols.
    assert_eq!(restored.to_string(), original.to_string());
}

#[test]
fn test_link_error_round_trips_through_json() {
    let mut obj = Object::new("o");
    obj.section(".data", [0u8; 8]);
    obj.relocate(".data", 0, "absent", Width::U64, 0);

    let err: LinkError = link(&[obj]).expect_err("the target is undefined");

    let json = serde_json::to_string(&err).expect("serialization succeeds");
    let restored: LinkError = serde_json::from_str(&json).expect("deserialization succeeds");

    assert_eq!(restored, err);
}
