//! The boundary between byte offsets and everything outside this crate.
//!
//! `docs/UNICODE.md` §1 names conflating the three coordinate spaces as the
//! most likely serious bug in the project, and explains why it is so hard to
//! catch: on ASCII all three agree, so every fixture written on a US keyboard
//! passes. The bug surfaces later, on a Tamil translator's machine, as a
//! cursor in the wrong place.
//!
//! The defence is structural rather than careful review. [`ByteSpan`] has no
//! `Serialize` impl, so a byte offset cannot cross to JavaScript. [`Char16`]
//! has one, and can only be produced by [`Utf16Mapper`]. Converting is
//! therefore the single narrow path out, and it is this module.
//!
//! [`ByteSpan`]: crate::ByteSpan

use crate::lines::LineIndex;
use crate::span::ByteSpan;

/// An offset in UTF-16 code units — what JavaScript, CodeMirror, and DOM
/// ranges count in.
///
/// The inner value is private, which is a deliberate departure from the sketch
/// in UNICODE §1. That sketch writes `pub u32` while its prose says only
/// `Utf16Mapper` may construct one, and those cannot both hold: a public field
/// is a construction path. The prose is the part that matters, so the field is
/// sealed and `Utf16Mapper` is the only way in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct Char16(u32);

impl Char16 {
    /// The offset as a number, for arithmetic and for tests.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// An offset that arrived **from** the editor.
    ///
    /// The sealed constructor exists to stop a byte offset masquerading as a
    /// Char16 one — the failure that is invisible on ASCII and wrong
    /// everywhere else. It does not exist to stop offsets entering from
    /// outside, and there is one place they legitimately do: CodeMirror and
    /// DOM ranges already count in UTF-16 code units, so a position reported
    /// by the editor *is* a Char16 and only needs carrying.
    ///
    /// Named so the distinction is visible at every call site. If this ever
    /// appears somewhere that is not translating an editor position, that is
    /// the bug — a plain `From<u32>` would have made it unfindable.
    pub const fn from_editor(offset: u32) -> Self {
        Self(offset)
    }
}

impl std::fmt::Display for Char16 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A half-open range in UTF-16 code units. The form every span takes once it
/// leaves this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct Char16Range {
    pub start: Char16,
    pub end: Char16,
}

impl Char16Range {
    pub const fn len(&self) -> u32 {
        self.end.0.saturating_sub(self.start.0)
    }

    pub const fn is_empty(&self) -> bool {
        self.end.0 <= self.start.0
    }
}

/// Converts byte offsets into UTF-16 code-unit offsets.
///
/// Indexed by line, because that is the shape of the question actually asked:
/// offsets cluster around the cursor and around the chunk being reparsed, so a
/// binary search to the line followed by a short scan within it beats both a
/// full scan and a per-character table. A table would cost four bytes per
/// character of the document — for a 2 MB file, more than the file.
///
/// # The source must be the one that was indexed
///
/// The mapper stores line offsets, not text, so every method takes the source
/// back. Passing a *different* string returns nonsense rather than failing,
/// which is exactly the class of bug this module exists to prevent — so the
/// length is checked, and the methods return `None` on a mismatch.
/// [`Document`] holds a mapper alongside the source it was built from and
/// exposes the conversions directly; prefer that.
///
/// [`Document`]: crate::Document
#[derive(Debug, Clone)]
pub struct Utf16Mapper {
    /// The byte half of the line table. Shared rather than duplicated, so a
    /// caller wanting a line number does not have to come through here — see
    /// [`LineIndex`].
    lines: LineIndex,
    /// UTF-16 column at each line start, parallel to `lines`. Begins with `0`.
    line_char16: Vec<u32>,
    len_char16: u32,
}

impl Utf16Mapper {
    /// Builds the line index. One pass over the source.
    pub fn new(source: &str) -> Self {
        let mut line_char16 = vec![0u32];
        let mut char16 = 0u32;

        for character in source.chars() {
            char16 += character.len_utf16() as u32;
            if character == '\n' {
                line_char16.push(char16);
            }
        }

        Self {
            lines: LineIndex::new(source),
            line_char16,
            len_char16: char16,
        }
    }

    /// The byte-only line table underneath.
    ///
    /// Here so that asking "which line?" does not require a UTF-16
    /// conversion. A consumer with no JavaScript boundary — a compositor
    /// printing `book.usfm:42:7` — wants this and not [`Char16`].
    pub const fn lines(&self) -> &LineIndex {
        &self.lines
    }

    /// Length of the indexed source, in UTF-16 code units.
    pub const fn len_char16(&self) -> Char16 {
        Char16(self.len_char16)
    }

    /// Number of lines in the indexed source.
    pub fn line_count(&self) -> usize {
        self.lines.line_count()
    }

    fn matches(&self, source: &str) -> bool {
        self.lines.matches(source)
    }

    /// Converts a byte offset.
    ///
    /// A byte past the end clamps to the end, and a byte landing *inside* a
    /// character resolves to that character's start. Both are deliberate:
    /// together they make the function total and monotonic, so a malformed
    /// span degrades to a cursor in nearly the right place rather than a
    /// panic. Well-formed spans never exercise either — the fuzz target
    /// (P0.11) asserts every offset the parser emits is in bounds and on a
    /// character boundary.
    ///
    /// Returns `None` only if `source` is not the string this mapper indexed.
    pub fn to_char16(&self, source: &str, byte: usize) -> Option<Char16> {
        if !self.matches(source) {
            return None;
        }
        let byte = byte.min(source.len());

        let line = self.lines.index_of(byte);
        let (line_byte, line_char16) = (self.lines.start_at(line), self.line_char16[line]);

        let mut char16 = line_char16;
        for (offset, character) in source[line_byte as usize..].char_indices() {
            let absolute = line_byte as usize + offset;
            // Stops on reaching the byte, and also when the byte falls within
            // this character, which is what rounds down to its start.
            if absolute + character.len_utf8() > byte {
                break;
            }
            char16 += character.len_utf16() as u32;
        }

        Some(Char16(char16))
    }

    /// The 1-based line a byte offset falls on.
    ///
    /// The line index is already here, so answering costs a binary search
    /// rather than a scan. It exists because a diagnostics panel has to say
    /// *where*, and working that out on the JavaScript side would mean walking
    /// the document on every keystroke to recover something this structure
    /// already knows.
    ///
    /// Returns `None` only if `source` is not the string this mapper indexed.
    pub fn line(&self, source: &str, byte: usize) -> Option<u32> {
        if !self.matches(source) {
            return None;
        }
        let byte = byte.min(source.len());

        Some(self.lines.line(byte))
    }

    /// Converts back to a byte offset.
    ///
    /// `None` if the offset is past the end, or if it falls **between the two
    /// halves of a surrogate pair** — that position names no character and has
    /// no byte offset, so there is nothing honest to return. A caller seeing
    /// `None` here has an offset that JavaScript itself would consider
    /// interior to a character.
    pub fn to_byte(&self, source: &str, offset: Char16) -> Option<usize> {
        if !self.matches(source) || offset.0 > self.len_char16 {
            return None;
        }
        let target = offset.0;

        let line = self
            .line_char16
            .partition_point(|line_char16| *line_char16 <= target)
            .saturating_sub(1);
        let (line_byte, line_char16) = (self.lines.start_at(line), self.line_char16[line]);

        let mut char16 = line_char16;
        for (byte, character) in source[line_byte as usize..].char_indices() {
            match char16.cmp(&target) {
                std::cmp::Ordering::Equal => return Some(line_byte as usize + byte),
                // Stepped over it, so the target was inside a surrogate pair.
                std::cmp::Ordering::Greater => return None,
                std::cmp::Ordering::Less => char16 += character.len_utf16() as u32,
            }
        }

        (char16 == target).then_some(source.len())
    }

    /// Converts a span. The operation almost every caller actually wants.
    pub fn to_char16_range(&self, source: &str, span: &ByteSpan) -> Option<Char16Range> {
        Some(Char16Range {
            start: self.to_char16(source, span.start)?,
            end: self.to_char16(source, span.end)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_offsets_are_unchanged() {
        let source = "\\id GEN\n\\c 1\n";
        let mapper = Utf16Mapper::new(source);
        for byte in 0..=source.len() {
            assert_eq!(mapper.to_char16(source, byte).unwrap().get(), byte as u32);
        }
    }

    #[test]
    fn a_multibyte_character_counts_as_one_unit() {
        // Tamil "க" is three UTF-8 bytes and one UTF-16 unit.
        let source = "க";
        let mapper = Utf16Mapper::new(source);
        assert_eq!(mapper.to_char16(source, 0).unwrap().get(), 0);
        assert_eq!(mapper.to_char16(source, 3).unwrap().get(), 1);
    }

    #[test]
    fn an_astral_character_counts_as_two_units() {
        // U+1D400 is four UTF-8 bytes and a surrogate pair in UTF-16.
        let source = "\u{1D400}";
        let mapper = Utf16Mapper::new(source);
        assert_eq!(mapper.to_char16(source, 0).unwrap().get(), 0);
        assert_eq!(mapper.to_char16(source, 4).unwrap().get(), 2);
    }

    #[test]
    fn an_offset_inside_a_surrogate_pair_has_no_byte_offset() {
        let source = "\u{1D400}";
        let mapper = Utf16Mapper::new(source);
        assert_eq!(mapper.to_byte(source, Char16(0)), Some(0));
        // Between the high and low surrogate. JavaScript would call this
        // interior to the character too.
        assert_eq!(mapper.to_byte(source, Char16(1)), None);
        assert_eq!(mapper.to_byte(source, Char16(2)), Some(4));
    }

    #[test]
    fn a_byte_inside_a_character_rounds_down_to_its_start() {
        let source = "கb";
        let mapper = Utf16Mapper::new(source);
        for byte in 0..3 {
            assert_eq!(mapper.to_char16(source, byte).unwrap().get(), 0, "{byte}");
        }
        assert_eq!(mapper.to_char16(source, 3).unwrap().get(), 1);
    }

    #[test]
    fn a_byte_past_the_end_clamps_rather_than_panicking() {
        let source = "abc";
        let mapper = Utf16Mapper::new(source);
        assert_eq!(mapper.to_char16(source, 9_999).unwrap().get(), 3);
    }

    #[test]
    fn the_wrong_source_is_refused_rather_than_answered() {
        let mapper = Utf16Mapper::new("\\c 1\n\\v 1 text\n");
        assert_eq!(mapper.to_char16("something else entirely", 3), None);
    }

    #[test]
    fn a_byte_offset_reports_the_line_it_falls_on() {
        // Tamil on the second line, so the answer cannot come from counting
        // UTF-16 units and calling them bytes.
        let source = "\\id GEN\n\\v 1 க்ஷேமம்\n\\v 2 x\n";
        let mapper = Utf16Mapper::new(source);

        assert_eq!(mapper.line(source, 0), Some(1));
        assert_eq!(mapper.line(source, 7), Some(1)); // the newline itself
        assert_eq!(mapper.line(source, 8), Some(2)); // just after it
                                                     // Inside the Tamil, at a byte well past where the UTF-16 offset for
                                                     // the same character would be — 21 bytes of Tamil are 7 units.
        assert_eq!(mapper.line(source, 20), Some(2));
        assert_eq!(mapper.line(source, source.find("\\v 2").unwrap()), Some(3));
        // The source ends with a newline, so there is an empty fourth line and
        // the end of the file is on it. The same convention as the editor's,
        // which matters because these two numbers sit beside each other in the
        // panel and the gutter.
        assert_eq!(mapper.line(source, source.len()), Some(4));
    }

    #[test]
    fn a_line_past_the_end_clamps_rather_than_panicking() {
        let mapper = Utf16Mapper::new("abc\n");
        assert_eq!(Utf16Mapper::new("abc\n").line("abc\n", 9_999), Some(2));
        assert_eq!(mapper.line("something else", 0), None);
    }

    #[test]
    fn offsets_are_indexed_per_line() {
        let source = "க\nb\nc\n";
        let mapper = Utf16Mapper::new(source);
        assert_eq!(mapper.line_count(), 4);
        // Line 2 starts after "க\n" — 4 bytes, 2 units.
        assert_eq!(mapper.to_char16(source, 4).unwrap().get(), 2);
    }
}
