//! The normalization index — finding text that is spelled differently.
//!
//! UNICODE §4 states the problem exactly: text circulates in both NFC and NFD,
//! keyboards and export tools disagree, and the same word can have two byte
//! representations that render identically. Meanwhile the buffer is **never**
//! normalized, because a file in NFD must save as NFD.
//!
//! > a search for a word gets zero hits because the keyboard produced NFC and
//! > the file is NFD, with the word visibly on screen. That is the most
//! > infuriating bug this class of application can have.
//!
//! So: **normalize for comparison, never for storage.** This index holds an
//! NFC copy alongside the raw text with an offset map back to it. Searching
//! happens in the copy; results are reported in raw coordinates, and the
//! buffer is never touched.
//!
//! # Why the map is per segment rather than per byte
//!
//! Composition happens within a *starter and the marks that follow it* — `e` +
//! U+0301 becomes `é`, three bytes becoming two, but nothing composes across
//! the next starter. Those segments are therefore the natural unit: the map
//! holds one row each rather than one per byte, and a search result snaps to
//! whole characters, which is what a user selecting a match expects anyway.

use unicode_normalization::char::canonical_combining_class;
use unicode_normalization::{is_nfc, UnicodeNormalization};

use crate::ByteSpan;

/// One starter and its following marks, in both spellings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segment {
    raw_start: u32,
    raw_len: u32,
    normalized_start: u32,
    normalized_len: u32,
}

/// An NFC view of some text, with the map back to it.
#[derive(Debug, Clone, Default)]
pub struct NormalizedIndex {
    normalized: String,
    segments: Vec<Segment>,
    already_nfc: bool,
}

impl NormalizedIndex {
    /// Builds the index. One pass; allocates a second copy of the text.
    pub fn build(source: &str) -> Self {
        let already_nfc = is_nfc(source);
        let mut normalized = String::with_capacity(source.len());
        let mut segments = Vec::new();

        for (raw_start, piece) in split_at_starters(source) {
            let normalized_start = normalized.len();
            normalized.extend(piece.nfc());

            segments.push(Segment {
                raw_start: raw_start as u32,
                raw_len: piece.len() as u32,
                normalized_start: normalized_start as u32,
                normalized_len: (normalized.len() - normalized_start) as u32,
            });
        }

        Self {
            normalized,
            segments,
            already_nfc,
        }
    }

    /// The NFC text searches run against.
    pub fn text(&self) -> &str {
        &self.normalized
    }

    /// Whether the source was already NFC throughout.
    ///
    /// `false` is what `USFM-I021` reports — and only reports. UNICODE §4:
    /// normalization is offered as a command, never applied automatically,
    /// because the file's spelling is the file's business.
    pub fn is_normalized(&self) -> bool {
        self.already_nfc
    }

    /// Every occurrence of `query`, in **raw** coordinates.
    ///
    /// The query is normalized too, so an NFC query finds NFD text and the
    /// other way round. Matches snap outwards to whole segments, so a hit
    /// always covers complete characters rather than half of a composed one.
    pub fn find(&self, query: &str) -> Vec<ByteSpan> {
        if query.is_empty() {
            return Vec::new();
        }
        let needle: String = query.nfc().collect();

        let mut spans = Vec::new();
        let mut from = 0usize;
        while let Some(offset) = self.normalized[from..].find(&needle) {
            let start = from + offset;
            let end = start + needle.len();
            if let Some(span) = self.to_raw(start, end) {
                spans.push(span);
            }
            // Overlapping matches are not wanted; advance past this one.
            from = end.max(start + 1);
            if from >= self.normalized.len() {
                break;
            }
        }
        spans
    }

    /// Maps a range in the normalized text back to the raw text.
    fn to_raw(&self, start: usize, end: usize) -> Option<ByteSpan> {
        let first = self.segment_at(start)?;
        let last = self.segment_at(end.saturating_sub(1))?;

        Some(ByteSpan::new(
            self.segments[first].raw_start as usize,
            (self.segments[last].raw_start + self.segments[last].raw_len) as usize,
        ))
    }

    /// The segment containing a normalized offset.
    fn segment_at(&self, offset: usize) -> Option<usize> {
        if self.segments.is_empty() {
            return None;
        }
        let index = self
            .segments
            .partition_point(|segment| (segment.normalized_start as usize) <= offset)
            .checked_sub(1)?;
        Some(index)
    }

    /// The raw offset a normalized offset corresponds to, snapped to the start
    /// of its segment.
    pub fn raw_offset(&self, normalized_offset: usize) -> Option<usize> {
        self.segment_at(normalized_offset)
            .map(|index| self.segments[index].raw_start as usize)
    }
}

/// Splits text before each starter — a character whose canonical combining
/// class is zero.
///
/// This is the unit NFC composes within. Splitting anywhere else would let a
/// mark be separated from the base it composes onto, and the two halves would
/// then normalize differently from the whole.
fn split_at_starters(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut boundaries: Vec<usize> = text
        .char_indices()
        .filter(|(offset, character)| *offset == 0 || canonical_combining_class(*character) == 0)
        .map(|(offset, _)| offset)
        .collect();

    if boundaries.first() != Some(&0) && !text.is_empty() {
        boundaries.insert(0, 0);
    }

    (0..boundaries.len()).map(move |index| {
        let start = boundaries[index];
        let end = boundaries.get(index + 1).copied().unwrap_or(text.len());
        (start, &text[start..end])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "café" with the accent composed, then decomposed.
    const NFC: &str = "caf\u{e9}";
    const NFD: &str = "cafe\u{301}";

    #[test]
    fn the_two_spellings_really_do_differ_in_bytes() {
        // If this ever stops holding the rest of the module is pointless.
        assert_ne!(NFC.as_bytes(), NFD.as_bytes());
        assert_eq!(NFC.chars().count(), 4);
        assert_eq!(NFD.chars().count(), 5);
    }

    #[test]
    fn an_nfc_query_finds_nfd_text() {
        // The bug UNICODE §4 calls the most infuriating this application can
        // have: the word is on screen and the search reports nothing.
        let index = NormalizedIndex::build(NFD);
        let hits = index.find(NFC);

        assert_eq!(hits.len(), 1, "NFC query found no NFD text");
        assert_eq!(hits[0].slice(NFD), Some(NFD));
    }

    #[test]
    fn an_nfd_query_finds_nfc_text() {
        let index = NormalizedIndex::build(NFC);
        let hits = index.find(NFD);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].slice(NFC), Some(NFC));
    }

    #[test]
    fn the_raw_text_is_never_altered() {
        // Storage is untouched; only the comparison copy is normalized.
        let index = NormalizedIndex::build(NFD);
        assert_ne!(index.text(), NFD);
        assert_eq!(index.text(), NFC);
    }

    #[test]
    fn matches_report_raw_offsets() {
        let source = "\\v 1 cafe\u{301} and more cafe\u{301}\n";
        let index = NormalizedIndex::build(source);
        let hits = index.find("caf\u{e9}");

        assert_eq!(hits.len(), 2);
        for hit in hits {
            assert_eq!(hit.slice(source), Some("cafe\u{301}"));
        }
    }

    #[test]
    fn a_mixed_document_is_reported_and_a_clean_one_is_not() {
        assert!(!NormalizedIndex::build(NFD).is_normalized());
        assert!(NormalizedIndex::build(NFC).is_normalized());
        assert!(NormalizedIndex::build("plain ascii").is_normalized());
    }

    #[test]
    fn complex_scripts_survive_the_round_trip() {
        // Devanagari and Tamil conjuncts, Hebrew with points, Arabic.
        for source in [
            "क्षि",
            "க்ஷேமம்",
            "בְּרֵאשִׁית",
            "مرحبا",
            "\u{1D400}",
            "e\u{301}q\u{323}\u{307}",
        ] {
            let index = NormalizedIndex::build(source);
            let hits = index.find(source);
            assert_eq!(hits.len(), 1, "{source:?} did not find itself");
            assert_eq!(hits[0].slice(source), Some(source), "{source:?}");
        }
    }

    #[test]
    fn every_segment_maps_back_inside_the_source() {
        let source = "\\v 1 cafe\u{301} க்ஷ שלום \u{1D400}\n";
        let index = NormalizedIndex::build(source);

        for offset in 0..index.text().len() {
            if !index.text().is_char_boundary(offset) {
                continue;
            }
            let raw = index.raw_offset(offset).expect("a mapping");
            assert!(raw <= source.len());
            assert!(
                source.is_char_boundary(raw),
                "raw offset {raw} is mid-character"
            );
        }
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert!(NormalizedIndex::build("text").find("").is_empty());
    }

    #[test]
    fn an_empty_source_is_handled() {
        let index = NormalizedIndex::build("");
        assert!(index.find("anything").is_empty());
        assert!(index.is_normalized());
    }
}
