//! The input to a link: an [`Object`] and the pieces it is built from.

use alloc::string::String;
use alloc::vec::Vec;

/// The size of the address a [relocation](Object::relocate) patches in.
///
/// A relocation writes a resolved address into a section as a little-endian integer of
/// this width. Pick the width that matches the slot the producer left: a 32-bit pointer
/// table uses [`U32`](Width::U32), a 64-bit one uses [`U64`](Width::U64). The link fails
/// with [`RelocationOverflow`](crate::LinkError::RelocationOverflow) if a resolved address
/// does not fit the chosen width.
///
/// # Examples
///
/// ```
/// use linker_lang::Width;
///
/// assert_eq!(Width::U32.bytes(), 4);
/// assert_eq!(Width::U64.bytes(), 8);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Width {
    /// A 32-bit address: four little-endian bytes.
    U32,
    /// A 64-bit address: eight little-endian bytes.
    U64,
}

impl Width {
    /// Returns the number of bytes a relocation of this width occupies.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::Width;
    ///
    /// assert_eq!(Width::U32.bytes(), 4);
    /// assert_eq!(Width::U64.bytes(), 8);
    /// ```
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Width::U32 => 4,
            Width::U64 => 8,
        }
    }

    /// The largest unsigned value this width can hold, as an `i128` so an addend can push a
    /// candidate address above it and still be compared without wrapping.
    pub(crate) const fn max_value(self) -> i128 {
        match self {
            Width::U32 => u32::MAX as i128,
            Width::U64 => u64::MAX as i128,
        }
    }
}

/// A named run of bytes contributed by an object — the unit sections merge by.
pub(crate) struct Section {
    pub(crate) name: String,
    pub(crate) data: Vec<u8>,
}

/// A symbol definition: `name` lives at `offset` inside the object's section `section`.
pub(crate) struct SymbolDef {
    pub(crate) name: String,
    pub(crate) section: String,
    pub(crate) offset: u64,
}

/// A relocation: the `width` bytes at `offset` in `section` must hold the address of
/// `target`, plus `addend`.
pub(crate) struct Relocation {
    pub(crate) section: String,
    pub(crate) offset: u64,
    pub(crate) target: String,
    pub(crate) width: Width,
    pub(crate) addend: i64,
}

/// One compilation unit handed to the linker: named byte sections, the symbols defined in
/// them, and the relocations still to be patched.
///
/// An object is the linker's input island — the form a backend or an object-file reader
/// produces. Build one with [`new`](Object::new), append bytes with
/// [`section`](Object::section), mark addresses with [`define`](Object::define), and
/// record holes to fill with [`relocate`](Object::relocate). The object carries no
/// addresses of its own; [`link`](crate::link) assigns them when it lays every object out
/// together.
///
/// Sections are addressed by name. Appending to a name that already exists extends that
/// section rather than starting a new one, so an object holds at most one section per
/// name.
///
/// # Examples
///
/// Build an object with a `.text` section and a symbol at its start:
///
/// ```
/// use linker_lang::Object;
///
/// let mut obj = Object::new("greeter");
/// obj.section(".text", b"\xc3".to_vec()); // a one-byte `ret`
/// obj.define("greet", ".text", 0);
///
/// assert_eq!(obj.name(), "greeter");
/// ```
pub struct Object {
    pub(crate) name: String,
    pub(crate) sections: Vec<Section>,
    pub(crate) symbols: Vec<SymbolDef>,
    pub(crate) relocations: Vec<Relocation>,
}

impl Object {
    /// Creates an empty object with the given name.
    ///
    /// The name identifies the object in diagnostics — for example, which object a
    /// [`RelocationOutOfRange`](crate::LinkError::RelocationOutOfRange) came from — and is
    /// not otherwise linked into the image.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::Object;
    ///
    /// let obj = Object::new("module.o");
    /// assert_eq!(obj.name(), "module.o");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Object {
            name: name.into(),
            sections: Vec::new(),
            symbols: Vec::new(),
            relocations: Vec::new(),
        }
    }

    /// Returns the object's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Appends `data` to the section named `name`, creating the section if it is new, and
    /// returns the offset within that section where `data` begins.
    ///
    /// The returned offset is the natural anchor for a symbol or relocation you place next:
    /// it is `0` for a fresh section and the previous length when extending one. Sections
    /// with the same name across objects merge at link time in object order.
    ///
    /// # Examples
    ///
    /// Two appends build one section; the second starts where the first ended:
    ///
    /// ```
    /// use linker_lang::Object;
    ///
    /// let mut obj = Object::new("data");
    /// assert_eq!(obj.section(".data", [1, 2, 3, 4]), 0);
    /// assert_eq!(obj.section(".data", [5, 6]), 4); // appended after the first chunk
    /// ```
    pub fn section(&mut self, name: impl Into<String>, data: impl Into<Vec<u8>>) -> u64 {
        let name = name.into();
        let data = data.into();
        if let Some(section) = self.sections.iter_mut().find(|s| s.name == name) {
            let start = section.data.len() as u64;
            section.data.extend_from_slice(&data);
            start
        } else {
            self.sections.push(Section { name, data });
            0
        }
    }

    /// Defines a symbol named `name` at `offset` bytes into the section `section`.
    ///
    /// The symbol resolves to an absolute address once the image is laid out. Defining the
    /// same name in two objects is a [`DuplicateSymbol`](crate::LinkError::DuplicateSymbol)
    /// error at link time; naming a section the object never created is an
    /// [`InvalidSection`](crate::LinkError::InvalidSection) error.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::Object;
    ///
    /// let mut obj = Object::new("code");
    /// obj.section(".text", [0u8; 16]);
    /// obj.define("entry", ".text", 0);
    /// obj.define("helper", ".text", 8);
    /// ```
    pub fn define(&mut self, name: impl Into<String>, section: impl Into<String>, offset: u64) {
        self.symbols.push(SymbolDef {
            name: name.into(),
            section: section.into(),
            offset,
        });
    }

    /// Records a relocation: the `width` bytes at `offset` in `section` hold the address of
    /// `target`, plus `addend`.
    ///
    /// At link time the target is resolved to its address, `addend` is added, and the
    /// result is written into the section's bytes little-endian. A target no object defines
    /// is an [`UndefinedSymbol`](crate::LinkError::UndefinedSymbol); a slot that runs past
    /// the section's bytes is a [`RelocationOutOfRange`](crate::LinkError::RelocationOutOfRange);
    /// a resolved value too large for `width` is a
    /// [`RelocationOverflow`](crate::LinkError::RelocationOverflow).
    ///
    /// # Examples
    ///
    /// A pointer slot that should hold the address of `entry`, and a second that should
    /// hold the address eight bytes past it:
    ///
    /// ```
    /// use linker_lang::{Object, Width};
    ///
    /// let mut obj = Object::new("vectors");
    /// obj.section(".data", [0u8; 16]);
    /// obj.relocate(".data", 0, "entry", Width::U64, 0);
    /// obj.relocate(".data", 8, "entry", Width::U64, 8);
    /// ```
    pub fn relocate(
        &mut self,
        section: impl Into<String>,
        offset: u64,
        target: impl Into<String>,
        width: Width,
        addend: i64,
    ) {
        self.relocations.push(Relocation {
            section: section.into(),
            offset,
            target: target.into(),
            width,
            addend,
        });
    }

    /// The bytes of the section named `name`, or `None` if the object has no such section.
    pub(crate) fn section_data(&self, name: &str) -> Option<&[u8]> {
        self.sections
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.data.as_slice())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests assert on known values; a missing one should fail the test loudly"
)]
mod tests {
    use super::{Object, Width};

    #[test]
    fn test_width_bytes_match_the_variant() {
        assert_eq!(Width::U32.bytes(), 4);
        assert_eq!(Width::U64.bytes(), 8);
    }

    #[test]
    fn test_new_object_is_named_and_empty() {
        let obj = Object::new("unit");
        assert_eq!(obj.name(), "unit");
        assert!(obj.sections.is_empty());
        assert!(obj.symbols.is_empty());
        assert!(obj.relocations.is_empty());
    }

    #[test]
    fn test_section_returns_zero_for_a_fresh_section() {
        let mut obj = Object::new("o");
        assert_eq!(obj.section(".text", [1, 2, 3]), 0);
    }

    #[test]
    fn test_section_appends_to_an_existing_name_and_returns_the_join_offset() {
        let mut obj = Object::new("o");
        assert_eq!(obj.section(".data", [0u8; 4]), 0);
        assert_eq!(obj.section(".data", [9u8; 2]), 4);
        // The two appends became one six-byte section.
        assert_eq!(obj.section_data(".data").unwrap().len(), 6);
        assert_eq!(obj.sections.len(), 1);
    }

    #[test]
    fn test_distinct_names_make_distinct_sections() {
        let mut obj = Object::new("o");
        let _ = obj.section(".text", [0u8; 3]);
        let _ = obj.section(".data", [0u8; 5]);
        assert_eq!(obj.sections.len(), 2);
        assert_eq!(obj.section_data(".text").unwrap().len(), 3);
        assert_eq!(obj.section_data(".data").unwrap().len(), 5);
    }

    #[test]
    fn test_section_data_is_none_for_an_unknown_name() {
        let obj = Object::new("o");
        assert!(obj.section_data(".bss").is_none());
    }
}
