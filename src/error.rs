//! The error a link reports when it cannot produce an image.

use alloc::string::String;
use core::fmt;

use crate::object::Width;

/// The reason a [`link`](crate::link) could not be completed.
///
/// A link can fail for a handful of distinct reasons: two objects claim the same symbol,
/// a relocation or entry point names a symbol nobody defines, a relocation's slot does not
/// fit the section it is in or the address does not fit the slot, or the laid-out image
/// would run past the address space. Each case points at the object, symbol, or section
/// involved so the producer can be fixed.
///
/// The set is `#[non_exhaustive]`: a future linking feature (a new relocation kind, a new
/// layout constraint) may add a variant, so a `match` on this type must include a wildcard
/// arm.
///
/// # Examples
///
/// ```
/// use linker_lang::{link, LinkError, Object};
///
/// // Two objects both define `main`.
/// let mut a = Object::new("a");
/// a.section(".text", [0u8; 1]);
/// a.define("main", ".text", 0);
///
/// let mut b = Object::new("b");
/// b.section(".text", [0u8; 1]);
/// b.define("main", ".text", 0);
///
/// match link(&[a, b]) {
///     Err(LinkError::DuplicateSymbol { name }) => assert_eq!(name, "main"),
///     other => panic!("expected a DuplicateSymbol error, got {other:?}"),
/// }
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum LinkError {
    /// A symbol of this name is defined by more than one object. A symbol's name is its
    /// identity across the link, so two definitions are ambiguous; rename one or drop the
    /// duplicate definition.
    DuplicateSymbol {
        /// The name defined more than once.
        name: String,
    },

    /// A relocation names a `target` symbol that no object defines. The wrapped `object`
    /// is the one that requested the relocation; define the symbol or remove the reference.
    UndefinedSymbol {
        /// The unresolved symbol name.
        name: String,
        /// The object whose relocation referenced it.
        object: String,
    },

    /// The [entry point](crate::Linker::entry) the linker was configured with is not
    /// defined by any object. Define the symbol, or link without an entry point.
    UndefinedEntry {
        /// The configured entry symbol name.
        name: String,
    },

    /// A symbol or relocation names a section the object never created with
    /// [`Object::section`](crate::Object::section). The name is almost certainly a typo for
    /// a real section.
    InvalidSection {
        /// The object that named the section.
        object: String,
        /// The section name that does not exist in that object.
        section: String,
    },

    /// A relocation's slot — `offset` plus its [`Width`] — runs past the end of the section
    /// it patches. The producer recorded an offset the section's bytes do not cover.
    RelocationOutOfRange {
        /// The object that requested the relocation.
        object: String,
        /// The section the slot is in.
        section: String,
        /// The byte offset of the slot within the object's section.
        offset: u64,
    },

    /// A relocation resolved to an address that does not fit its [`Width`] — it is negative
    /// after the addend, or larger than the width can hold. Widen the slot or adjust the
    /// layout so the address fits.
    RelocationOverflow {
        /// The symbol whose resolved address overflowed.
        target: String,
        /// The width the address had to fit.
        width: Width,
    },

    /// The laid-out image would extend past the end of the 64-bit address space. The base
    /// address plus the total section size does not fit in a `u64`.
    LayoutOverflow,
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::DuplicateSymbol { name } => {
                write!(f, "symbol `{name}` is defined by more than one object")
            }
            LinkError::UndefinedSymbol { name, object } => {
                write!(
                    f,
                    "object `{object}` references symbol `{name}`, which no object defines"
                )
            }
            LinkError::UndefinedEntry { name } => {
                write!(f, "entry point `{name}` is not defined by any object")
            }
            LinkError::InvalidSection { object, section } => {
                write!(
                    f,
                    "object `{object}` names section `{section}`, which it never created"
                )
            }
            LinkError::RelocationOutOfRange {
                object,
                section,
                offset,
            } => {
                write!(
                    f,
                    "relocation at offset {offset} in section `{section}` of object \
                     `{object}` runs past the end of the section"
                )
            }
            LinkError::RelocationOverflow { target, width } => {
                write!(
                    f,
                    "the resolved address of `{target}` does not fit in {} bytes",
                    width.bytes()
                )
            }
            LinkError::LayoutOverflow => {
                f.write_str("the laid-out image would exceed the 64-bit address space")
            }
        }
    }
}

impl core::error::Error for LinkError {}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests assert on specific outcomes; a wrong outcome should fail the test loudly"
)]
mod tests {
    use super::LinkError;
    use crate::object::Width;
    use alloc::string::ToString;

    #[test]
    fn test_each_variant_renders_a_distinct_message() {
        let messages = [
            LinkError::DuplicateSymbol {
                name: "main".into(),
            }
            .to_string(),
            LinkError::UndefinedSymbol {
                name: "puts".into(),
                object: "a".into(),
            }
            .to_string(),
            LinkError::UndefinedEntry {
                name: "_start".into(),
            }
            .to_string(),
            LinkError::InvalidSection {
                object: "a".into(),
                section: ".txet".into(),
            }
            .to_string(),
            LinkError::RelocationOutOfRange {
                object: "a".into(),
                section: ".data".into(),
                offset: 12,
            }
            .to_string(),
            LinkError::RelocationOverflow {
                target: "big".into(),
                width: Width::U32,
            }
            .to_string(),
            LinkError::LayoutOverflow.to_string(),
        ];
        // No two variants share a message, and each names its subject.
        for (i, a) in messages.iter().enumerate() {
            for b in &messages[i + 1..] {
                assert_ne!(a, b);
            }
        }
        assert!(messages[0].contains("main"));
        assert!(messages[5].contains("4 bytes"));
    }

    #[test]
    fn test_error_is_a_std_error() {
        fn assert_error<E: core::error::Error>(_: &E) {}
        assert_error(&LinkError::LayoutOverflow);
    }
}
