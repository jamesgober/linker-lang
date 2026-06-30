//! Laying objects out into an image: the [`Linker`] and the [`link`] shortcut.
//!
//! The link is a few linear passes over the input. First the sections of every object are
//! concatenated by name into output sections, recording where each object's contribution
//! lands. Then the output sections are placed at addresses, end to end from the base
//! address. With addresses known, every symbol resolves to one and goes into the table —
//! a second definition of a name is a [`DuplicateSymbol`]. Finally each relocation is
//! resolved against that table and its address written into the section bytes. The entry
//! point, if configured, is the last symbol looked up.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::LinkError;
use crate::image::{Image, OutputSection};
use crate::object::{Object, Width};

/// A configured linker: a base address and an optional entry point.
///
/// `Linker` is the configurable form of [`link`]. Set the address the image is laid out
/// from with [`base_address`](Linker::base_address) and the symbol execution starts at
/// with [`entry`](Linker::entry), then call [`link`](Linker::link). The defaults — base
/// address `0`, no entry point — are what the free [`link`] function uses, so reach for
/// `Linker` only when you need to change one of them.
///
/// A linker holds no per-link state, so one instance links many sets of objects.
///
/// # Examples
///
/// ```
/// use linker_lang::{Linker, Object};
///
/// let mut obj = Object::new("o");
/// obj.section(".text", [0u8; 16]);
/// obj.define("_start", ".text", 0);
///
/// let image = Linker::new()
///     .base_address(0x40_0000)
///     .entry("_start")
///     .link(&[obj])
///     .expect("the entry point is defined");
///
/// assert_eq!(image.symbol("_start"), Some(0x40_0000));
/// assert_eq!(image.entry(), Some(0x40_0000));
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Linker {
    base_address: u64,
    entry: Option<String>,
}

impl Linker {
    /// Creates a linker with the default configuration: base address `0` and no entry
    /// point. Equivalent to `Linker::default()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the address the first output section is placed at; later sections follow it.
    ///
    /// Every resolved symbol address is relative to this base, so a non-zero base shifts
    /// the whole image — useful when the image will be loaded at a known address.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::{Linker, Object};
    ///
    /// let mut obj = Object::new("o");
    /// obj.section(".text", [0u8; 4]);
    /// obj.define("f", ".text", 0);
    ///
    /// let image = Linker::new().base_address(0x1000).link(&[obj]).unwrap();
    /// assert_eq!(image.symbol("f"), Some(0x1000));
    /// ```
    #[must_use]
    pub const fn base_address(mut self, address: u64) -> Self {
        self.base_address = address;
        self
    }

    /// Sets the symbol whose address becomes the image's [`entry`](Image::entry).
    ///
    /// The symbol must be defined by one of the linked objects; if it is not, the link
    /// fails with [`LinkError::UndefinedEntry`]. Without this, the image has no entry point.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::{Linker, Object};
    ///
    /// let mut obj = Object::new("o");
    /// obj.section(".text", [0u8; 4]);
    /// obj.define("main", ".text", 0);
    ///
    /// let image = Linker::new().entry("main").link(&[obj]).unwrap();
    /// assert_eq!(image.entry(), Some(0));
    /// ```
    #[must_use]
    pub fn entry(mut self, symbol: impl Into<String>) -> Self {
        self.entry = Some(symbol.into());
        self
    }

    /// Links `objects` into an [`Image`].
    ///
    /// The objects' sections are merged by name, every symbol is resolved to an address,
    /// and every relocation is patched. The objects are consumed by reference and not
    /// modified.
    ///
    /// # Errors
    ///
    /// Returns a [`LinkError`] if a symbol is defined twice, a relocation or the entry
    /// point names a symbol no object defines, a symbol or relocation names a section its
    /// object never created, a relocation's slot runs past its section or its address does
    /// not fit the relocation width, or the laid-out image would exceed the address space.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::{Linker, Object, Width};
    ///
    /// let mut code = Object::new("code");
    /// code.section(".text", [0u8; 8]);
    /// code.define("handler", ".text", 0);
    ///
    /// let mut table = Object::new("table");
    /// table.section(".data", [0u8; 8]);
    /// table.relocate(".data", 0, "handler", Width::U64, 0);
    ///
    /// let image = Linker::new().link(&[code, table]).unwrap();
    /// let slot = image.section(".data").unwrap().data();
    /// assert_eq!(u64::from_le_bytes(slot.try_into().unwrap()), 0); // patched with handler's address
    /// ```
    pub fn link(&self, objects: &[Object]) -> Result<Image, LinkError> {
        let (mut sections, contrib) = merge_sections(objects)?;
        place_sections(&mut sections, self.base_address)?;
        let symbols = resolve_symbols(objects, &sections, &contrib)?;
        patch_relocations(objects, &mut sections, &symbols, &contrib)?;
        let entry = resolve_entry(self.entry.as_deref(), &symbols)?;

        Ok(Image {
            sections,
            symbols,
            entry,
        })
    }
}

/// Maps each output section name to its index in the output list. Built during merging and
/// consulted while resolving and patching. Borrows the section names from the objects.
type SectionIndex<'a> = BTreeMap<&'a str, usize>;

/// Per-object record of where that object's contribution to each section name begins within
/// the merged output section.
type Contributions<'a> = Vec<BTreeMap<&'a str, u64>>;

/// Concatenates the sections of every object by name, in object order, into a list of
/// output sections (addresses not yet assigned). Returns the merged sections and, for each
/// object, the offset its contribution to each section begins at.
fn merge_sections(
    objects: &[Object],
) -> Result<(Vec<OutputSection>, Contributions<'_>), LinkError> {
    let mut index: SectionIndex<'_> = BTreeMap::new();
    let mut sections: Vec<OutputSection> = Vec::new();
    let mut contrib: Contributions<'_> = Vec::with_capacity(objects.len());

    for object in objects {
        let mut object_contrib: BTreeMap<&str, u64> = BTreeMap::new();
        for section in &object.sections {
            let name = section.name.as_str();
            let slot = match index.get(name) {
                Some(&i) => i,
                None => {
                    let i = sections.len();
                    index.insert(name, i);
                    sections.push(OutputSection {
                        name: section.name.clone(),
                        address: 0,
                        data: Vec::new(),
                    });
                    i
                }
            };
            let base = sections[slot].data.len() as u64;
            sections[slot].data.extend_from_slice(&section.data);
            object_contrib.insert(name, base);
        }
        contrib.push(object_contrib);
    }

    Ok((sections, contrib))
}

/// Assigns each output section an address, placing them end to end from `base_address`.
fn place_sections(sections: &mut [OutputSection], base_address: u64) -> Result<(), LinkError> {
    let mut cursor = base_address;
    for section in sections.iter_mut() {
        section.address = cursor;
        cursor = cursor
            .checked_add(section.data.len() as u64)
            .ok_or(LinkError::LayoutOverflow)?;
    }
    Ok(())
}

/// Resolves every defined symbol to an absolute address, rejecting a name defined twice or
/// a symbol placed in a section its object never created.
fn resolve_symbols(
    objects: &[Object],
    sections: &[OutputSection],
    contrib: &Contributions<'_>,
) -> Result<BTreeMap<String, u64>, LinkError> {
    let index = section_index(sections);
    let mut symbols: BTreeMap<String, u64> = BTreeMap::new();

    for (object, object_contrib) in objects.iter().zip(contrib) {
        for symbol in &object.symbols {
            let address = symbol_address(
                object,
                object_contrib,
                &index,
                sections,
                &symbol.section,
                symbol.offset,
            )?;
            if symbols.insert(symbol.name.clone(), address).is_some() {
                return Err(LinkError::DuplicateSymbol {
                    name: symbol.name.clone(),
                });
            }
        }
    }

    Ok(symbols)
}

/// Patches every relocation: resolves its target, checks the slot fits and the address fits
/// the width, and writes the address into the section bytes little-endian.
fn patch_relocations(
    objects: &[Object],
    sections: &mut [OutputSection],
    symbols: &BTreeMap<String, u64>,
    contrib: &Contributions<'_>,
) -> Result<(), LinkError> {
    // Owned keys so this map does not borrow `sections`, which is mutated below.
    let index: BTreeMap<String, usize> = sections
        .iter()
        .enumerate()
        .map(|(i, section)| (section.name.clone(), i))
        .collect();

    for (object, object_contrib) in objects.iter().zip(contrib) {
        for relocation in &object.relocations {
            let base = section_base(object, object_contrib, &relocation.section)?;
            let slot = index
                .get(relocation.section.as_str())
                .copied()
                .ok_or_else(|| LinkError::InvalidSection {
                    object: object.name.clone(),
                    section: relocation.section.clone(),
                })?;

            // The slot must fit within this object's own contribution to the section.
            let section_len = object
                .section_data(&relocation.section)
                .map_or(0, <[u8]>::len) as u64;
            let end = relocation
                .offset
                .checked_add(relocation.width.bytes() as u64)
                .filter(|&end| end <= section_len)
                .ok_or_else(|| LinkError::RelocationOutOfRange {
                    object: object.name.clone(),
                    section: relocation.section.clone(),
                    offset: relocation.offset,
                })?;

            let target = symbols
                .get(relocation.target.as_str())
                .copied()
                .ok_or_else(|| LinkError::UndefinedSymbol {
                    name: relocation.target.clone(),
                    object: object.name.clone(),
                })?;

            let value = i128::from(target) + i128::from(relocation.addend);
            if value < 0 || value > relocation.width.max_value() {
                return Err(LinkError::RelocationOverflow {
                    target: relocation.target.clone(),
                    width: relocation.width,
                });
            }

            // `base + offset + width <= base + section_len <= output length`, so the write
            // is in bounds by construction.
            let start = (base + relocation.offset) as usize;
            let bytes = &mut sections[slot].data[start..(base + end) as usize];
            match relocation.width {
                Width::U32 => bytes.copy_from_slice(&(value as u32).to_le_bytes()),
                Width::U64 => bytes.copy_from_slice(&(value as u64).to_le_bytes()),
            }
        }
    }

    Ok(())
}

/// Looks up the entry-point symbol, if one was configured.
fn resolve_entry(
    entry: Option<&str>,
    symbols: &BTreeMap<String, u64>,
) -> Result<Option<u64>, LinkError> {
    match entry {
        Some(name) => symbols
            .get(name)
            .copied()
            .map(Some)
            .ok_or_else(|| LinkError::UndefinedEntry { name: name.into() }),
        None => Ok(None),
    }
}

/// Rebuilds the name-to-index map over already-merged sections.
fn section_index(sections: &[OutputSection]) -> SectionIndex<'_> {
    sections
        .iter()
        .enumerate()
        .map(|(i, section)| (section.name.as_str(), i))
        .collect()
}

/// The offset within its merged output section where `object`'s contribution to `section`
/// begins, or [`InvalidSection`](LinkError::InvalidSection) if it never created that section.
fn section_base(
    object: &Object,
    object_contrib: &BTreeMap<&str, u64>,
    section: &str,
) -> Result<u64, LinkError> {
    object_contrib
        .get(section)
        .copied()
        .ok_or_else(|| LinkError::InvalidSection {
            object: object.name.clone(),
            section: section.into(),
        })
}

/// Resolves a symbol or its anchor to an absolute address: section address + contribution
/// base + offset.
fn symbol_address(
    object: &Object,
    object_contrib: &BTreeMap<&str, u64>,
    index: &SectionIndex<'_>,
    sections: &[OutputSection],
    section: &str,
    offset: u64,
) -> Result<u64, LinkError> {
    let base = section_base(object, object_contrib, section)?;
    let slot = index
        .get(section)
        .copied()
        .ok_or_else(|| LinkError::InvalidSection {
            object: object.name.clone(),
            section: section.into(),
        })?;
    sections[slot]
        .address
        .checked_add(base)
        .and_then(|a| a.checked_add(offset))
        .ok_or(LinkError::LayoutOverflow)
}

/// Links `objects` into an [`Image`] with the default configuration.
///
/// A shortcut for `Linker::new().link(objects)`: base address `0` and no entry point. Use
/// [`Linker`] when you need to set either.
///
/// # Errors
///
/// Returns a [`LinkError`] for any of the conditions documented on [`Linker::link`].
///
/// # Examples
///
/// ```
/// use linker_lang::{link, Object};
///
/// let mut a = Object::new("a");
/// a.section(".text", [1, 2, 3, 4]);
/// a.define("a_start", ".text", 0);
///
/// let mut b = Object::new("b");
/// b.section(".text", [5, 6]);
/// b.define("b_start", ".text", 0);
///
/// let image = link(&[a, b]).unwrap();
///
/// // `.text` is the two objects' contributions, in order; `b_start` follows a's four bytes.
/// assert_eq!(image.section(".text").unwrap().data(), &[1, 2, 3, 4, 5, 6]);
/// assert_eq!(image.symbol("a_start"), Some(0));
/// assert_eq!(image.symbol("b_start"), Some(4));
/// ```
pub fn link(objects: &[Object]) -> Result<Image, LinkError> {
    Linker::new().link(objects)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests build known-valid links, so they cannot fail"
)]
mod tests {
    use super::{Linker, link};
    use crate::error::LinkError;
    use crate::object::{Object, Width};

    #[test]
    fn test_same_named_sections_merge_in_object_order() {
        let mut a = Object::new("a");
        a.section(".text", [1, 2]);
        let mut b = Object::new("b");
        b.section(".text", [3, 4, 5]);

        let image = link(&[a, b]).unwrap();
        assert_eq!(image.section(".text").unwrap().data(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_sections_are_placed_end_to_end_from_the_base() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 4]);
        obj.section(".data", [0u8; 8]);

        let image = Linker::new().base_address(0x1000).link(&[obj]).unwrap();
        assert_eq!(image.section(".text").unwrap().address(), 0x1000);
        assert_eq!(image.section(".data").unwrap().address(), 0x1004);
    }

    #[test]
    fn test_symbol_resolves_to_section_plus_contribution_plus_offset() {
        let mut a = Object::new("a");
        a.section(".text", [0u8; 4]);
        a.define("a_fn", ".text", 2);
        let mut b = Object::new("b");
        b.section(".text", [0u8; 4]);
        b.define("b_fn", ".text", 1);

        let image = Linker::new().base_address(0x10).link(&[a, b]).unwrap();
        assert_eq!(image.symbol("a_fn"), Some(0x12));
        assert_eq!(image.symbol("b_fn"), Some(0x10 + 4 + 1));
    }

    #[test]
    fn test_relocation_is_patched_with_the_target_address() {
        let mut code = Object::new("code");
        code.section(".text", [0u8; 8]);
        code.define("target", ".text", 4);

        let mut data = Object::new("data");
        data.section(".data", [0u8; 4]);
        data.relocate(".data", 0, "target", Width::U32, 0);

        let image = Linker::new()
            .base_address(0x100)
            .link(&[code, data])
            .unwrap();
        let slot = image.section(".data").unwrap().data();
        assert_eq!(u32::from_le_bytes(slot.try_into().unwrap()), 0x104);
    }

    #[test]
    fn test_relocation_addend_is_added_to_the_address() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 8]);
        obj.define("base", ".text", 0);
        obj.section(".data", [0u8; 8]);
        obj.relocate(".data", 0, "base", Width::U64, 16);

        let image = Linker::new().base_address(0x1000).link(&[obj]).unwrap();
        let slot = image.section(".data").unwrap().data();
        // `.text` is at 0x1000, so base = 0x1000; plus addend 16 = 0x1010.
        assert_eq!(u64::from_le_bytes(slot.try_into().unwrap()), 0x1010);
    }

    #[test]
    fn test_duplicate_symbol_is_rejected() {
        let mut a = Object::new("a");
        a.section(".text", [0u8; 1]);
        a.define("dup", ".text", 0);
        let mut b = Object::new("b");
        b.section(".text", [0u8; 1]);
        b.define("dup", ".text", 0);

        assert_eq!(
            link(&[a, b]),
            Err(LinkError::DuplicateSymbol { name: "dup".into() })
        );
    }

    #[test]
    fn test_undefined_relocation_target_is_rejected() {
        let mut obj = Object::new("o");
        obj.section(".data", [0u8; 8]);
        obj.relocate(".data", 0, "nowhere", Width::U64, 0);

        assert_eq!(
            link(&[obj]),
            Err(LinkError::UndefinedSymbol {
                name: "nowhere".into(),
                object: "o".into(),
            })
        );
    }

    #[test]
    fn test_undefined_entry_is_rejected() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 1]);

        assert_eq!(
            Linker::new().entry("_start").link(&[obj]),
            Err(LinkError::UndefinedEntry {
                name: "_start".into()
            })
        );
    }

    #[test]
    fn test_symbol_in_a_missing_section_is_rejected() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 4]);
        obj.define("ghost", ".rodata", 0); // never created `.rodata`

        assert_eq!(
            link(&[obj]),
            Err(LinkError::InvalidSection {
                object: "o".into(),
                section: ".rodata".into(),
            })
        );
    }

    #[test]
    fn test_relocation_past_the_section_end_is_rejected() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 4]);
        obj.define("t", ".text", 0);
        obj.section(".data", [0u8; 4]);
        obj.relocate(".data", 1, "t", Width::U64, 0); // 1 + 8 > 4

        assert_eq!(
            link(&[obj]),
            Err(LinkError::RelocationOutOfRange {
                object: "o".into(),
                section: ".data".into(),
                offset: 1,
            })
        );
    }

    #[test]
    fn test_address_too_large_for_width_is_rejected() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 4]);
        obj.define("t", ".text", 0);
        obj.section(".data", [0u8; 4]);
        obj.relocate(".data", 0, "t", Width::U32, 0);

        // Base above u32::MAX makes the resolved address overflow a 32-bit slot.
        assert_eq!(
            Linker::new().base_address(0x1_0000_0000).link(&[obj]),
            Err(LinkError::RelocationOverflow {
                target: "t".into(),
                width: Width::U32,
            })
        );
    }

    #[test]
    fn test_negative_addend_below_zero_is_rejected() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 4]);
        obj.define("t", ".text", 0); // address 0
        obj.section(".data", [0u8; 8]);
        obj.relocate(".data", 0, "t", Width::U64, -1); // 0 + (-1) < 0

        assert_eq!(
            link(&[obj]),
            Err(LinkError::RelocationOverflow {
                target: "t".into(),
                width: Width::U64,
            })
        );
    }

    #[test]
    fn test_negative_addend_within_range_resolves() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 8]);
        obj.define("t", ".text", 4); // address 0x104 with base 0x100
        obj.section(".data", [0u8; 8]);
        obj.relocate(".data", 0, "t", Width::U64, -4);

        let image = Linker::new().base_address(0x100).link(&[obj]).unwrap();
        let slot = image.section(".data").unwrap().data();
        assert_eq!(u64::from_le_bytes(slot.try_into().unwrap()), 0x100);
    }

    #[test]
    fn test_link_is_deterministic() {
        let build = || {
            let mut a = Object::new("a");
            a.section(".text", [1, 2, 3]);
            a.define("a", ".text", 0);
            let mut b = Object::new("b");
            b.section(".data", [4, 5]);
            b.define("b", ".data", 1);
            [a, b]
        };
        assert_eq!(link(&build()).unwrap(), link(&build()).unwrap());
    }
}
