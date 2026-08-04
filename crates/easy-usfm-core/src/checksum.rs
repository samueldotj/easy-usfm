//! The checksum that catches a mirror drifting from the editor.
//!
//! ARCHITECTURE §9: the worker holds a copy of the document and the editor is
//! authoritative. If the two ever disagree, **every offset in the interface is
//! wrong** — diagnostics land on the wrong words, clicking a verse jumps to
//! the wrong line, and nothing about the display says so. The checksum is what
//! turns that from an unfalsifiable worry into a detectable event.
//!
//! # Why not xxh3
//!
//! ARCHITECTURE names xxh3. That is a good choice for Rust and a bad one for
//! the other half of this comparison: the main thread has to compute the same
//! value over the same text in JavaScript, and xxh3 there means shipping a
//! second WASM module or a hand-ported implementation to keep in step. Either
//! would be a new place for the two sides to disagree, in the mechanism whose
//! entire job is detecting disagreement.
//!
//! FNV-1a over UTF-16 code units is used instead. It is weaker as a hash and
//! entirely adequate here — this is detecting accidental drift, not resisting
//! an adversary — and it is four lines in both languages, over the unit both
//! languages already count in. `src/lib/checksum.ts` is the other half, and
//! `agrees_with_the_typescript_implementation` pins them together.

/// FNV-1a, 32-bit, over UTF-16 code units.
pub fn checksum(text: &str) -> u32 {
    const OFFSET: u32 = 2_166_136_261;
    const PRIME: u32 = 16_777_619;

    let mut hash = OFFSET;
    for unit in text.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_empty_document_hashes_to_the_offset_basis() {
        assert_eq!(checksum(""), 2_166_136_261);
    }

    #[test]
    fn different_text_hashes_differently() {
        assert_ne!(checksum("\\v 1 a"), checksum("\\v 1 b"));
        // Transposition, which a simple sum would miss entirely.
        assert_ne!(checksum("ab"), checksum("ba"));
    }

    #[test]
    fn it_counts_utf16_units_not_bytes() {
        // An astral character is one code point, four UTF-8 bytes, and two
        // UTF-16 units. Hashing bytes would give a value JavaScript cannot
        // reproduce without decoding the whole document differently.
        let astral = "\u{1D400}";
        assert_eq!(astral.encode_utf16().count(), 2);
        assert_eq!(astral.len(), 4);
        assert_ne!(checksum(astral), checksum("\u{FFFD}\u{FFFD}"));
    }

    /// The vectors `src/lib/checksum.test.ts` asserts.
    ///
    /// Two implementations in two languages agreeing by inspection is how they
    /// stop agreeing six months later. These constants are the contract: if
    /// either side changes, one of the two test files fails.
    const VECTORS: &[(&str, u32)] = &[
        ("", 0x811C_9DC5),
        ("a", 0xE40C_292C),
        ("\\id GEN\n", 0x8D05_171E),
        // Tamil conjunct: three code points, nine bytes, seven UTF-16 units.
        ("க்ஷேமம்", 0xCBC8_4650),
        // Astral: one code point, two UTF-16 units. The case that separates a
        // UTF-16 implementation from a byte one.
        ("\u{1D400}", 0x9ADB_D370),
        // Right-to-left text and a CRLF, since the checksum runs over the
        // editor's text and must not care about either.
        ("\\v 1 שלום\r\n", 0x4C49_3CE0),
    ];

    #[test]
    fn agrees_with_the_typescript_implementation() {
        for (text, expected) in VECTORS {
            assert_eq!(
                checksum(text),
                *expected,
                "{text:?} disagrees with src/lib/checksum.test.ts"
            );
        }
    }
}
