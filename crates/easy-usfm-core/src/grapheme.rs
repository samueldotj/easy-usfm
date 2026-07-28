//! Grapheme clusters — the unit the user believes they are editing.
//!
//! UNICODE §2: Tamil `க்ஷ` is three code points rendering as one indivisible
//! unit, and Devanagari `क्ष` likewise. Arrow keys, selection, and the column
//! in the status bar all move in these, not in code points.
//!
//! Only what the offset work needs is here. Cursor movement is CodeMirror's
//! job, and the grapheme-aware interaction rules land with the editor; this
//! module exists so the Rust side can be checked against the browser's
//! `Intl.Segmenter`, which is UNICODE §9.1's fourth property and the one that
//! catches a disagreement before it becomes a cursor that appears stuck.

use unicode_segmentation::UnicodeSegmentation;

/// Byte offsets of every grapheme-cluster boundary, including `0` and the
/// length of the text.
///
/// Boundaries rather than the clusters themselves: the comparison against
/// `Intl.Segmenter` is about where the breaks fall, and offsets are what the
/// rest of the engine speaks.
pub fn boundaries(text: &str) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(text.len() / 2 + 1);
    offsets.push(0);
    for (offset, cluster) in text.grapheme_indices(true) {
        offsets.push(offset + cluster.len());
    }
    offsets
}

/// How many grapheme clusters precede `byte` within `line`.
///
/// The status-bar column. UNICODE §2 counts columns in clusters, so a
/// conjunct that the user sees as one character advances the column by one
/// however many code points it took to write.
///
/// A byte inside a cluster counts that cluster as not yet passed, which keeps
/// the column stable while the cursor sits within a conjunct.
pub fn column(line: &str, byte: usize) -> usize {
    line.grapheme_indices(true)
        .take_while(|(offset, cluster)| offset + cluster.len() <= byte)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_boundaries_fall_between_every_character() {
        assert_eq!(boundaries("abc"), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_devanagari_conjunct_is_one_cluster() {
        // क + ् + ष. The Devanagari virama carries Indic_Conjunct_Break=Linker,
        // so Unicode 15.1's rule GB9c holds the sequence together.
        let text = "क्ष";
        assert_eq!(text.chars().count(), 3);
        assert_eq!(boundaries(text), vec![0, text.len()]);
    }

    #[test]
    fn a_tamil_conjunct_is_two_clusters_despite_rendering_as_one() {
        // க + ் + ஷ renders as a single indivisible unit, and UNICODE §2 cites
        // it as the example of exactly that. It is nonetheless **two**
        // extended grapheme clusters, because GB9c only applies to viramas
        // marked Indic_Conjunct_Break=Linker and the Tamil virama U+0BCD is
        // not one of them — unlike its Devanagari counterpart above.
        //
        // This is not our bug to fix and not a segmentation error. It matters
        // because UNICODE §2 makes the grapheme cluster the unit for arrow
        // keys and selection, so on this text one press of the arrow key
        // stops *inside* what the reader sees as one character. Recorded here
        // rather than asserted away, since the editor will have to decide
        // what to do about it.
        let text = "க்ஷ";
        assert_eq!(text.chars().count(), 3);
        assert_eq!(
            boundaries(text),
            vec![0, 6, 9],
            "Tamil conjunct segmentation changed — check the Unicode version"
        );
    }

    #[test]
    fn a_combining_mark_joins_its_base() {
        // "e" + U+0301, which renders as é.
        let text = "e\u{301}";
        assert_eq!(boundaries(text), vec![0, text.len()]);
    }

    #[test]
    fn columns_count_clusters_not_code_points() {
        // Devanagari, where GB9c does hold the conjunct together: nine bytes,
        // three code points, one column.
        let line = "क्षa";
        assert_eq!(line.chars().count(), 4);
        assert_eq!(column(line, 0), 0);
        assert_eq!(column(line, 3), 0);
        assert_eq!(column(line, 6), 0);
        assert_eq!(column(line, 9), 1);
        assert_eq!(column(line, 10), 2);
    }
}
