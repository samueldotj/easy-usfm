//! Byte offsets, and the reason they cannot leave this crate.
//!
//! `docs/UNICODE.md` §1 defines three coordinate spaces — byte, Char16, and
//! grapheme — and names conflating them as the most likely serious bug in the
//! project, precisely because ASCII fixtures cannot detect it: on ASCII all
//! three agree.
//!
//! This module owns the first space. `Char16` and the `Utf16Mapper` that
//! converts into it arrive with P0.3; until then nothing in this crate can
//! produce a frontend-facing offset, which is the intended state rather than a
//! gap.

use std::ops::Range;

/// A half-open byte range into the source text.
///
/// **Deliberately not `Serialize`.** A byte offset that reached JavaScript
/// would index a UTF-16 string and land in the wrong place — silently, and
/// only for non-ASCII text. Withholding the impl turns that class of mistake
/// into a compile error rather than a bug report from a Tamil translator. The
/// conversion at the boundary is `Utf16Mapper`'s job (P0.3), and it is the
/// only way to obtain an offset that may cross.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    pub const fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset < self.end
    }

    /// The source text this span covers, or `None` if the span is out of
    /// bounds or does not fall on character boundaries.
    ///
    /// Returning `Option` rather than slicing directly is what keeps a
    /// malformed span from panicking. The fuzz target (P0.11) asserts every
    /// span the parser emits is in bounds and on a boundary; this method is
    /// what makes that assertion cheap to write and a bad span survivable in
    /// the meantime.
    pub fn slice<'a>(&self, source: &'a str) -> Option<&'a str> {
        source.get(self.start..self.end)
    }
}

impl From<Range<usize>> for ByteSpan {
    fn from(range: Range<usize>) -> Self {
        Self::new(range.start, range.end)
    }
}

impl From<ByteSpan> for Range<usize> {
    fn from(span: ByteSpan) -> Self {
        span.start..span.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_returns_the_covered_text() {
        let source = "\\id GEN\n";
        assert_eq!(ByteSpan::new(0, 3).slice(source), Some("\\id"));
        assert_eq!(ByteSpan::new(4, 7).slice(source), Some("GEN"));
    }

    #[test]
    fn out_of_bounds_slice_is_none_rather_than_a_panic() {
        assert_eq!(ByteSpan::new(0, 99).slice("short"), None);
    }

    #[test]
    fn slice_off_a_character_boundary_is_none_rather_than_a_panic() {
        // Devanagari "कि" — the consonant alone is three bytes, so byte 1 is
        // mid-character. A naive implementation panics here.
        let source = "कि";
        assert_eq!(ByteSpan::new(0, 1).slice(source), None);
        assert_eq!(ByteSpan::new(0, 3).slice(source), Some("क"));
    }

    #[test]
    fn empty_and_reversed_spans_do_not_underflow() {
        assert!(ByteSpan::new(5, 5).is_empty());
        assert_eq!(ByteSpan::new(9, 4).len(), 0);
    }
}
