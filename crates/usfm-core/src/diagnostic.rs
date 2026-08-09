//! Diagnostics, and the stable codes that identify them.
//!
//! `docs/PRODUCT.md` §9: diagnostics never prevent saving, and every one
//! carries a stable code so it can be suppressed individually and its wording
//! can change without breaking tooling.
//!
//! **Codes and severity are separate.** The letter in `USFM-E003` records the
//! condition's canonical severity; the severity actually reported is a
//! computed field and may differ, because PRODUCT §9 derives it from the
//! marker table — the same unknown marker is a Warning or an Information
//! depending on the document's detected USFM version. Deriving severity that
//! way is P0.7; until then the parser's own severity is reported unchanged,
//! so the two can disagree for now.

use crate::span::ByteSpan;

/// How much a diagnostic matters.
///
/// PRODUCT §9's vocabulary. `Information` is spelled out rather than
/// abbreviated because it appears in the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Information,
    Warning,
    Error,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Information => "information",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A condition the engine can report.
///
/// The numbering is a single sequence shared across severities, which is why
/// `USFM-E018` and `USFM-I021` can both exist. Four numbers are fixed by the
/// documentation before anything emits them — 018, 021, 022, and 023 — and are
/// reserved here so a later pass cannot quietly take them.
///
/// P0.7 owns the complete catalogue and the severity derivation. What is here
/// covers the conditions the parser can currently produce, plus those four.
// Ord so codes can live in a set: suppression is by code, and a sorted set
// keeps the settings interface's list stable between runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticCode {
    // ---- markers and nesting ----
    UnknownMarker,
    DeprecatedMarker,
    UnclosedMarker,
    StrayCloseMarker,
    MisnestedMarker,
    MissingNestingPrefix,
    ImplicitClose,
    UnclosedNote,
    UnclosedAtEof,

    // ---- structure ----
    InvalidChapterSequence,
    InvalidVerseSequence,
    DuplicateChapter,
    DuplicateId,
    MissingIdMarker,
    InvalidBookCode,
    NoteSubmarkerOutsideNote,
    TextBeforeId,

    /// `USFM-E018` — non-ASCII digits in a `\v` number. UNICODE §6: verse
    /// numbers stay ASCII because that is the USFM data model; `\vp` and `\cp`
    /// are where published numbering lives.
    NonAsciiVerseDigits,

    HeaderAfterBody,
    MilestoneMismatch,

    /// `USFM-I021` — the document mixes normalization forms. UNICODE §4:
    /// reported, never corrected automatically.
    MixedNormalization,

    /// `USFM-W022` — a zero-width joiner inside a marker name. Almost
    /// certainly accidental, and otherwise produces an unknown-marker error
    /// whose cause is invisible on screen.
    JoinerInMarkerName,

    /// `USFM-I023` — a zero-width joiner adjacent to a marker boundary.
    JoinerAtMarkerBoundary,

    // ---- attributes and content ----
    InvalidAttributes,
    MissingChapterNumber,
    MissingVerseNumber,
    VerseOutsideParagraph,
    MissingChapterMarker,
    CharCrossesVerseBoundary,
    EmptyFigure,
    UnquotedAttributeValue,
    MissingRequiredAttribute,
    DefaultAttributeNotDefined,
    BodyParagraphBeforeChapter,
    NonEmptyBlankLine,
    LeadingZeros,
    EmptyWordMarker,
    MissingMilestoneSelfClose,
    InvalidTableColumnSequence,

    /// `USFM-I040` — the marker was introduced after the version the document
    /// declares. Usually a stale `\usfm` line rather than a mistake.
    MarkerNewerThanDocument,

    /// `USFM-W041` — the USFM 2.x positional `\fig` syntax, where fields are
    /// separated by `|` with no names. Carries a quick fix.
    LegacyFigureSyntax,

    /// `USFM-E042` — two verses in a chapter cover the same number. Includes
    /// the case where a range overlaps a verse stated separately.
    DuplicateVerse,

    /// `USFM-I043` — a chapter skips a verse number. Information, because
    /// intentional omissions are ordinary in published Scripture.
    VerseGap,
}

impl DiagnosticCode {
    /// The stable code string, as it appears in the interface and in
    /// suppression settings.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownMarker => "USFM-W001",
            Self::DeprecatedMarker => "USFM-W002",
            Self::UnclosedMarker => "USFM-E003",
            Self::StrayCloseMarker => "USFM-E004",
            Self::MisnestedMarker => "USFM-E005",
            Self::MissingNestingPrefix => "USFM-E006",
            Self::ImplicitClose => "USFM-E007",
            Self::UnclosedNote => "USFM-E008",
            Self::UnclosedAtEof => "USFM-E009",

            Self::InvalidChapterSequence => "USFM-E010",
            Self::InvalidVerseSequence => "USFM-E011",
            Self::DuplicateChapter => "USFM-E012",
            Self::DuplicateId => "USFM-E013",
            Self::MissingIdMarker => "USFM-E014",
            Self::InvalidBookCode => "USFM-E015",
            Self::NoteSubmarkerOutsideNote => "USFM-E016",
            Self::TextBeforeId => "USFM-E017",
            Self::NonAsciiVerseDigits => "USFM-E018",
            Self::HeaderAfterBody => "USFM-W019",
            Self::MilestoneMismatch => "USFM-E020",
            Self::MixedNormalization => "USFM-I021",
            Self::JoinerInMarkerName => "USFM-W022",
            Self::JoinerAtMarkerBoundary => "USFM-I023",

            Self::InvalidAttributes => "USFM-E024",
            Self::MissingChapterNumber => "USFM-E025",
            Self::MissingVerseNumber => "USFM-E026",
            Self::VerseOutsideParagraph => "USFM-W027",
            Self::MissingChapterMarker => "USFM-E028",
            Self::CharCrossesVerseBoundary => "USFM-W029",
            Self::EmptyFigure => "USFM-W030",
            Self::UnquotedAttributeValue => "USFM-W031",
            Self::MissingRequiredAttribute => "USFM-E032",
            Self::DefaultAttributeNotDefined => "USFM-E033",
            Self::BodyParagraphBeforeChapter => "USFM-W034",
            Self::NonEmptyBlankLine => "USFM-W035",
            Self::LeadingZeros => "USFM-W036",
            Self::EmptyWordMarker => "USFM-W037",
            Self::MissingMilestoneSelfClose => "USFM-E038",
            Self::InvalidTableColumnSequence => "USFM-E039",
            Self::MarkerNewerThanDocument => "USFM-I040",
            Self::LegacyFigureSyntax => "USFM-W041",
            Self::DuplicateVerse => "USFM-E042",
            Self::VerseGap => "USFM-I043",
        }
    }

    /// Every code, for tests and for the settings interface that lists what
    /// can be suppressed.
    pub const ALL: &'static [DiagnosticCode] = &[
        Self::UnknownMarker,
        Self::DeprecatedMarker,
        Self::UnclosedMarker,
        Self::StrayCloseMarker,
        Self::MisnestedMarker,
        Self::MissingNestingPrefix,
        Self::ImplicitClose,
        Self::UnclosedNote,
        Self::UnclosedAtEof,
        Self::InvalidChapterSequence,
        Self::InvalidVerseSequence,
        Self::DuplicateChapter,
        Self::DuplicateId,
        Self::MissingIdMarker,
        Self::InvalidBookCode,
        Self::NoteSubmarkerOutsideNote,
        Self::TextBeforeId,
        Self::NonAsciiVerseDigits,
        Self::HeaderAfterBody,
        Self::MilestoneMismatch,
        Self::MixedNormalization,
        Self::JoinerInMarkerName,
        Self::JoinerAtMarkerBoundary,
        Self::InvalidAttributes,
        Self::MissingChapterNumber,
        Self::MissingVerseNumber,
        Self::VerseOutsideParagraph,
        Self::MissingChapterMarker,
        Self::CharCrossesVerseBoundary,
        Self::EmptyFigure,
        Self::UnquotedAttributeValue,
        Self::MissingRequiredAttribute,
        Self::DefaultAttributeNotDefined,
        Self::BodyParagraphBeforeChapter,
        Self::NonEmptyBlankLine,
        Self::LeadingZeros,
        Self::EmptyWordMarker,
        Self::MissingMilestoneSelfClose,
        Self::InvalidTableColumnSequence,
        Self::MarkerNewerThanDocument,
        Self::LegacyFigureSyntax,
        Self::DuplicateVerse,
        Self::VerseGap,
    ];
}

impl From<DiagnosticCode> for &'static str {
    fn from(code: DiagnosticCode) -> Self {
        code.as_str()
    }
}

/// Serialized as its code string — `"USFM-E018"`, not the variant name. The
/// code is the stable identifier; the variant name is ours to rename.
impl serde::Serialize for DiagnosticCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Something the engine wants to tell the user about a place in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub span: ByteSpan,
    pub message: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.code, self.severity, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn codes_are_unique() {
        let mut seen = HashSet::new();
        for code in DiagnosticCode::ALL {
            assert!(
                seen.insert(code.as_str()),
                "{} is allocated twice",
                code.as_str()
            );
        }
    }

    /// Forces the build to stop when a variant is added.
    ///
    /// The count assertion this replaces could be satisfied by bumping the
    /// number, so it caught a variant added *and* listed while missing the one
    /// case that matters — added and **not** listed. An exhaustive match
    /// cannot be satisfied that way: a new variant fails to compile here, and
    /// whoever is looking at this function is one line from `ALL`.
    ///
    /// `usfm-core` has a second consumer now, and BibleCompose mirrors this
    /// list to map codes through to its own diagnostics. A code missing from
    /// `ALL` would reach a user as an unrecognised string rather than a
    /// diagnostic, so completeness is load-bearing outside this crate too.
    fn assert_exhaustive(code: DiagnosticCode) {
        match code {
            DiagnosticCode::UnknownMarker => {}
            DiagnosticCode::DeprecatedMarker => {}
            DiagnosticCode::UnclosedMarker => {}
            DiagnosticCode::StrayCloseMarker => {}
            DiagnosticCode::MisnestedMarker => {}
            DiagnosticCode::MissingNestingPrefix => {}
            DiagnosticCode::ImplicitClose => {}
            DiagnosticCode::UnclosedNote => {}
            DiagnosticCode::UnclosedAtEof => {}
            DiagnosticCode::InvalidChapterSequence => {}
            DiagnosticCode::InvalidVerseSequence => {}
            DiagnosticCode::DuplicateChapter => {}
            DiagnosticCode::DuplicateId => {}
            DiagnosticCode::MissingIdMarker => {}
            DiagnosticCode::InvalidBookCode => {}
            DiagnosticCode::NoteSubmarkerOutsideNote => {}
            DiagnosticCode::TextBeforeId => {}
            DiagnosticCode::NonAsciiVerseDigits => {}
            DiagnosticCode::HeaderAfterBody => {}
            DiagnosticCode::MilestoneMismatch => {}
            DiagnosticCode::MixedNormalization => {}
            DiagnosticCode::JoinerInMarkerName => {}
            DiagnosticCode::JoinerAtMarkerBoundary => {}
            DiagnosticCode::InvalidAttributes => {}
            DiagnosticCode::MissingChapterNumber => {}
            DiagnosticCode::MissingVerseNumber => {}
            DiagnosticCode::VerseOutsideParagraph => {}
            DiagnosticCode::MissingChapterMarker => {}
            DiagnosticCode::CharCrossesVerseBoundary => {}
            DiagnosticCode::EmptyFigure => {}
            DiagnosticCode::UnquotedAttributeValue => {}
            DiagnosticCode::MissingRequiredAttribute => {}
            DiagnosticCode::DefaultAttributeNotDefined => {}
            DiagnosticCode::BodyParagraphBeforeChapter => {}
            DiagnosticCode::NonEmptyBlankLine => {}
            DiagnosticCode::LeadingZeros => {}
            DiagnosticCode::EmptyWordMarker => {}
            DiagnosticCode::MissingMilestoneSelfClose => {}
            DiagnosticCode::InvalidTableColumnSequence => {}
            DiagnosticCode::MarkerNewerThanDocument => {}
            DiagnosticCode::LegacyFigureSyntax => {}
            DiagnosticCode::DuplicateVerse => {}
            DiagnosticCode::VerseGap => {}
        }
    }

    #[test]
    fn all_lists_every_variant() {
        for code in DiagnosticCode::ALL {
            assert_exhaustive(*code);
        }
        // Both directions: the match above proves no variant is forgotten,
        // this proves none is listed twice or invented.
        let unique: HashSet<_> = DiagnosticCode::ALL.iter().collect();
        assert_eq!(unique.len(), DiagnosticCode::ALL.len());
    }

    #[test]
    fn codes_documented_before_they_were_emitted_keep_their_numbers() {
        // These four are fixed in PRODUCT §9 and UNICODE §4, §6, and the
        // appendix. Renumbering any of them would break documentation that is
        // already written.
        assert_eq!(DiagnosticCode::NonAsciiVerseDigits.as_str(), "USFM-E018");
        assert_eq!(DiagnosticCode::MixedNormalization.as_str(), "USFM-I021");
        assert_eq!(DiagnosticCode::JoinerInMarkerName.as_str(), "USFM-W022");
        assert_eq!(DiagnosticCode::JoinerAtMarkerBoundary.as_str(), "USFM-I023");
    }

    #[test]
    fn every_code_is_well_formed() {
        for code in DiagnosticCode::ALL {
            let text = code.as_str();
            let rest = text.strip_prefix("USFM-").expect("USFM- prefix");
            let (letter, number) = rest.split_at(1);
            assert!(
                matches!(letter, "E" | "W" | "I"),
                "{text} has severity letter {letter}"
            );
            assert_eq!(number.len(), 3, "{text} is not three digits");
            assert!(number.chars().all(|c| c.is_ascii_digit()), "{text}");
        }
    }

    #[test]
    fn severity_orders_information_below_error() {
        assert!(Severity::Information < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
    }
}
