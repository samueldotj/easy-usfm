//! The token stream, for syntax highlighting.
//!
//! ARCHITECTURE §8.1 puts lexing on the cheap tier: per-keystroke work uses
//! `tokenize()` and never touches the tree. Highlighting is the thing that has
//! to keep up with typing, so it reads from here rather than from the parse.
//!
//! # Why not a regular expression
//!
//! Because the lexer already knows, and a second implementation would drift.
//! `\+bd` is a nested marker, `\bd*` is a closing one, `\qt-s` is a milestone,
//! and `|src="x"` is an attribute run — distinctions a pattern over the text
//! gets subtly wrong at exactly the places USFM is interesting. Highlighting
//! that disagrees with the parser is worse than none: it teaches the user a
//! model of the file that the diagnostics then contradict.

use crate::ByteSpan;

/// What a run of source is.
///
/// Deliberately coarser than the lexer's own vocabulary. This is what a
/// *reader* needs to tell apart (PRODUCT §4: markers, attributes, and text
/// distinguished); the finer distinctions are the parser's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenKind {
    /// `\p`, `\bd`, `\+it` — anything that opens a marker.
    Marker,
    /// `\bd*`, `\+it*` — anything that closes one.
    ClosingMarker,
    /// `\c` and its number.
    Chapter,
    /// `\v` and its number.
    Verse,
    /// `\qt-s`, `\ts-e`.
    Milestone,
    /// `|lemma="grace"` — the run after a `|`.
    Attributes,
    /// Everything else: the Scripture itself.
    Text,
}

/// One run of source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: ByteSpan,
}

/// Maps the lexer's vocabulary onto ours.
///
/// The lexer reports two levels: `kind` is the broad category — `marker`,
/// `closing_marker`, `attributes`, `text` — and `token_kind` the specific one,
/// which is where `chapter` and `verse` live. Reading only the first makes
/// `\c` and `\v` indistinguishable from `\p`, which loses exactly the two
/// markers worth finding by eye when scrolling through a book.
///
/// Anything unrecognised is Text: the safe default, since an unhighlighted run
/// reads as prose and most source is prose.
pub(crate) fn classify(kind: &str, token_kind: Option<&str>) -> TokenKind {
    match (kind, token_kind) {
        (_, Some("chapter")) => TokenKind::Chapter,
        (_, Some("verse")) => TokenKind::Verse,
        (_, Some("milestone")) => TokenKind::Milestone,
        ("milestone_end", _) => TokenKind::Milestone,
        ("closing_marker", _) => TokenKind::ClosingMarker,
        ("marker", _) => TokenKind::Marker,
        ("attributes", _) => TokenKind::Attributes,
        _ => TokenKind::Text,
    }
}

impl TokenKind {
    /// The CSS class the editor decorates with.
    ///
    /// Named rather than styled here: appearance lives in a stylesheet, never
    /// in an injected theme (SECURITY §5).
    pub const fn css_class(self) -> &'static str {
        match self {
            Self::Marker => "cm-usfm-marker",
            Self::ClosingMarker => "cm-usfm-marker-close",
            Self::Chapter => "cm-usfm-chapter",
            Self::Verse => "cm-usfm-verse",
            Self::Milestone => "cm-usfm-milestone",
            Self::Attributes => "cm-usfm-attribute",
            Self::Text => "cm-usfm-text",
        }
    }

    /// Whether this run must be bidi-isolated when rendered.
    ///
    /// UNICODE §8: a marker is Latin and left-to-right, and inside a
    /// right-to-left verse the surrounding text drags it to the wrong side of
    /// the line unless it is isolated. Rendering-only — no isolate characters
    /// are inserted, so the bytes are untouched and byte fidelity holds.
    pub const fn needs_bidi_isolation(self) -> bool {
        !matches!(self, Self::Text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lexers_vocabulary_maps_onto_ours() {
        assert_eq!(classify("marker", Some("regular")), TokenKind::Marker);
        assert_eq!(classify("marker", Some("nested")), TokenKind::Marker);
        assert_eq!(
            classify("closing_marker", Some("regular")),
            TokenKind::ClosingMarker
        );
        assert_eq!(classify("milestone_end", None), TokenKind::Milestone);
        assert_eq!(classify("attributes", None), TokenKind::Attributes);
    }

    #[test]
    fn chapter_and_verse_are_read_from_the_specific_kind() {
        // Both arrive with kind "marker"; only token_kind tells them apart.
        // Reading the broad category alone makes \c and \v look like \p.
        assert_eq!(classify("marker", Some("chapter")), TokenKind::Chapter);
        assert_eq!(classify("marker", Some("verse")), TokenKind::Verse);
        assert_eq!(classify("marker", Some("milestone")), TokenKind::Milestone);
    }

    #[test]
    fn anything_unrecognised_reads_as_prose() {
        // The safe default: an unhighlighted run looks like Scripture, which
        // is what most of the file is.
        assert_eq!(classify("whitespace", None), TokenKind::Text);
        assert_eq!(classify("something-new-upstream", None), TokenKind::Text);
    }

    #[test]
    fn every_marker_run_is_isolated_and_text_is_not() {
        for kind in [
            TokenKind::Marker,
            TokenKind::ClosingMarker,
            TokenKind::Chapter,
            TokenKind::Verse,
            TokenKind::Milestone,
            TokenKind::Attributes,
        ] {
            assert!(kind.needs_bidi_isolation(), "{kind:?}");
        }
        assert!(!TokenKind::Text.needs_bidi_isolation());
    }
}
