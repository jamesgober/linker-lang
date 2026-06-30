//! The output of a link: an [`Image`] and the [`OutputSection`]s it is laid out from.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// One laid-out section in a linked [`Image`]: a name, the address it begins at, and its
/// merged, relocated bytes.
///
/// An output section is the concatenation of every input section that shared its name,
/// in object order, placed at a fixed [`address`](OutputSection::address). Its
/// [`data`](OutputSection::data) is final — relocations into it have been patched — so it
/// is ready to be written to a file or loaded into memory at that address.
///
/// # Examples
///
/// ```
/// use linker_lang::{link, Object};
///
/// let mut obj = Object::new("o");
/// obj.section(".text", [1, 2, 3, 4]);
/// let image = link(&[obj]).unwrap();
///
/// let text = image.section(".text").unwrap();
/// assert_eq!(text.name(), ".text");
/// assert_eq!(text.address(), 0);
/// assert_eq!(text.data(), &[1, 2, 3, 4]);
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OutputSection {
    pub(crate) name: String,
    pub(crate) address: u64,
    pub(crate) data: Vec<u8>,
}

impl OutputSection {
    /// Returns the section's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the address the section begins at in the laid-out image.
    #[must_use]
    pub const fn address(&self) -> u64 {
        self.address
    }

    /// Returns the section's final bytes, with every relocation into it patched.
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the number of bytes in the section.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the section has no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// A linked image: the laid-out sections, the resolved symbol table, and the entry point.
///
/// An image is what [`link`](crate::link) produces on success. Its
/// [`sections`](Image::sections) are placed at fixed addresses with their relocations
/// patched; its symbol table maps every defined name to the address it resolved to; and
/// its [`entry`](Image::entry) is the address of the configured entry point, if any. Look
/// up an address with [`symbol`](Image::symbol), or read a section's bytes with
/// [`section`](Image::section).
///
/// The [`Display`](fmt::Display) implementation renders a readable link map: the entry
/// point, each section with its address and size, and the symbol table sorted by name.
///
/// # Examples
///
/// ```
/// use linker_lang::{Linker, Object};
///
/// let mut obj = Object::new("o");
/// obj.section(".text", [0u8; 8]);
/// obj.define("main", ".text", 0);
/// obj.define("loop", ".text", 4);
///
/// let image = Linker::new().entry("main").link(&[obj]).unwrap();
///
/// assert_eq!(image.symbol("main"), Some(0));
/// assert_eq!(image.symbol("loop"), Some(4));
/// assert_eq!(image.entry(), Some(0));
/// assert_eq!(image.symbols().count(), 2);
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Image {
    pub(crate) sections: Vec<OutputSection>,
    pub(crate) symbols: BTreeMap<String, u64>,
    pub(crate) entry: Option<u64>,
}

impl Image {
    /// Returns the laid-out sections, in address order.
    #[must_use]
    pub fn sections(&self) -> &[OutputSection] {
        &self.sections
    }

    /// Returns the section named `name`, or `None` if the image has no such section.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::{link, Object};
    ///
    /// let mut obj = Object::new("o");
    /// obj.section(".data", [7, 7]);
    /// let image = link(&[obj]).unwrap();
    ///
    /// assert!(image.section(".data").is_some());
    /// assert!(image.section(".bss").is_none());
    /// ```
    #[must_use]
    pub fn section(&self, name: &str) -> Option<&OutputSection> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Returns the resolved address of the symbol named `name`, or `None` if no object
    /// defined it.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::{link, Object};
    ///
    /// let mut obj = Object::new("o");
    /// obj.section(".text", [0u8; 4]);
    /// obj.define("f", ".text", 2);
    /// let image = link(&[obj]).unwrap();
    ///
    /// assert_eq!(image.symbol("f"), Some(2));
    /// assert_eq!(image.symbol("missing"), None);
    /// ```
    #[must_use]
    pub fn symbol(&self, name: &str) -> Option<u64> {
        self.symbols.get(name).copied()
    }

    /// Returns the symbol table as `(name, address)` pairs, sorted by name.
    ///
    /// # Examples
    ///
    /// ```
    /// use linker_lang::{link, Object};
    ///
    /// let mut obj = Object::new("o");
    /// obj.section(".text", [0u8; 8]);
    /// obj.define("b", ".text", 4);
    /// obj.define("a", ".text", 0);
    /// let image = link(&[obj]).unwrap();
    ///
    /// let names: Vec<&str> = image.symbols().map(|(name, _)| name).collect();
    /// assert_eq!(names, ["a", "b"]); // sorted, not insertion order
    /// ```
    pub fn symbols(&self) -> impl Iterator<Item = (&str, u64)> {
        self.symbols
            .iter()
            .map(|(name, &addr)| (name.as_str(), addr))
    }

    /// Returns the address of the entry point, or `None` if the link was not configured
    /// with one. See [`Linker::entry`](crate::Linker::entry).
    #[must_use]
    pub const fn entry(&self) -> Option<u64> {
        self.entry
    }

    /// Returns the number of sections in the image.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    /// Returns `true` if the image has no sections.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.entry {
            Some(addr) => writeln!(f, "entry = {addr:#018x}")?,
            None => writeln!(f, "entry = none")?,
        }
        for section in &self.sections {
            writeln!(
                f,
                "{} @ {:#018x} ({} bytes)",
                section.name,
                section.address,
                section.data.len()
            )?;
        }
        if !self.symbols.is_empty() {
            f.write_str("symbols:\n")?;
            for (name, addr) in &self.symbols {
                writeln!(f, "    {addr:#018x} {name}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests build known-valid links, so they cannot fail"
)]
mod tests {
    use crate::{Linker, Object, link};
    use alloc::string::ToString;
    use alloc::vec::Vec;

    #[test]
    fn test_output_section_accessors_report_layout() {
        let mut obj = Object::new("o");
        obj.section(".text", [1, 2, 3, 4]);
        let image = link(&[obj]).unwrap();

        let text = image.section(".text").unwrap();
        assert_eq!(text.name(), ".text");
        assert_eq!(text.address(), 0);
        assert_eq!(text.data(), &[1, 2, 3, 4]);
        assert_eq!(text.len(), 4);
        assert!(!text.is_empty());
    }

    #[test]
    fn test_symbols_iterate_in_name_order() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 8]);
        obj.define("zebra", ".text", 0);
        obj.define("alpha", ".text", 4);
        let image = link(&[obj]).unwrap();

        let names: Vec<&str> = image.symbols().map(|(n, _)| n).collect();
        assert_eq!(names, ["alpha", "zebra"]);
    }

    #[test]
    fn test_display_renders_a_link_map() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 4]);
        obj.define("main", ".text", 0);
        let image = Linker::new().entry("main").link(&[obj]).unwrap();

        let map = image.to_string();
        assert!(map.contains("entry = 0x0000000000000000"));
        assert!(map.contains(".text @ 0x0000000000000000 (4 bytes)"));
        assert!(map.contains("0x0000000000000000 main"));
    }

    #[test]
    fn test_display_without_entry_says_none() {
        let mut obj = Object::new("o");
        obj.section(".text", [0u8; 1]);
        let image = link(&[obj]).unwrap();
        assert!(image.to_string().contains("entry = none"));
    }

    #[test]
    fn test_empty_link_is_an_empty_image() {
        let image = link(&[]).unwrap();
        assert!(image.is_empty());
        assert_eq!(image.len(), 0);
        assert_eq!(image.entry(), None);
        assert_eq!(image.symbols().count(), 0);
    }
}
