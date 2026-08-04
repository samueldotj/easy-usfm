//! Parser diagnostics to ours.

use crate::{Diagnostic, DiagnosticCode, Severity};

pub(super) fn convert(diagnostic: &usfm3::diagnostics::Diagnostic) -> Diagnostic {
    Diagnostic {
        code: convert_code(diagnostic.code),
        // Reported as the parser found it. PRODUCT §9 derives severity from
        // the marker table and the document's detected version instead, which
        // is P0.7 — at that point this stops being a pass-through.
        severity: convert_severity(diagnostic.severity),
        span: diagnostic.span.clone().into(),
        message: diagnostic.message.clone(),
    }
}

fn convert_severity(severity: usfm3::diagnostics::Severity) -> Severity {
    match severity {
        usfm3::diagnostics::Severity::Info => Severity::Information,
        usfm3::diagnostics::Severity::Warning => Severity::Warning,
        usfm3::diagnostics::Severity::Error => Severity::Error,
    }
}

/// Exhaustive by construction: adding a variant upstream stops the build here
/// rather than silently mapping to a catch-all, which is how a new condition
/// would otherwise reach users with no code and no way to suppress it.
fn convert_code(code: usfm3::diagnostics::DiagnosticCode) -> DiagnosticCode {
    use usfm3::diagnostics::DiagnosticCode as Upstream;

    match code {
        Upstream::UnknownMarker => DiagnosticCode::UnknownMarker,
        Upstream::DeprecatedMarker => DiagnosticCode::DeprecatedMarker,
        Upstream::UnclosedMarker => DiagnosticCode::UnclosedMarker,
        Upstream::StrayCloseMarker => DiagnosticCode::StrayCloseMarker,
        Upstream::MisnestedMarker => DiagnosticCode::MisnestedMarker,
        Upstream::MissingNestingPrefix => DiagnosticCode::MissingNestingPrefix,
        Upstream::ImplicitClose => DiagnosticCode::ImplicitClose,
        Upstream::UnclosedNote => DiagnosticCode::UnclosedNote,
        Upstream::UnclosedAtEof => DiagnosticCode::UnclosedAtEof,

        Upstream::InvalidChapterSequence => DiagnosticCode::InvalidChapterSequence,
        Upstream::InvalidVerseSequence => DiagnosticCode::InvalidVerseSequence,
        Upstream::DuplicateChapter => DiagnosticCode::DuplicateChapter,
        Upstream::DuplicateId => DiagnosticCode::DuplicateId,
        Upstream::MissingIdMarker => DiagnosticCode::MissingIdMarker,
        Upstream::InvalidBookCode => DiagnosticCode::InvalidBookCode,
        Upstream::NoteSubmarkerOutsideNote => DiagnosticCode::NoteSubmarkerOutsideNote,
        Upstream::TextBeforeId => DiagnosticCode::TextBeforeId,
        Upstream::HeaderAfterBody => DiagnosticCode::HeaderAfterBody,
        Upstream::MilestoneMismatch => DiagnosticCode::MilestoneMismatch,

        Upstream::InvalidAttributes => DiagnosticCode::InvalidAttributes,
        Upstream::MissingChapterNumber => DiagnosticCode::MissingChapterNumber,
        Upstream::MissingVerseNumber => DiagnosticCode::MissingVerseNumber,
        Upstream::VerseOutsideParagraph => DiagnosticCode::VerseOutsideParagraph,
        Upstream::MissingChapterMarker => DiagnosticCode::MissingChapterMarker,
        Upstream::CharCrossesVerseBoundary => DiagnosticCode::CharCrossesVerseBoundary,
        Upstream::EmptyFigure => DiagnosticCode::EmptyFigure,
        Upstream::UnquotedAttributeValue => DiagnosticCode::UnquotedAttributeValue,
        Upstream::MissingRequiredAttribute => DiagnosticCode::MissingRequiredAttribute,
        Upstream::DefaultAttributeNotDefined => DiagnosticCode::DefaultAttributeNotDefined,
        Upstream::BodyParagraphBeforeChapter => DiagnosticCode::BodyParagraphBeforeChapter,
        Upstream::NonEmptyBlankLine => DiagnosticCode::NonEmptyBlankLine,
        Upstream::LeadingZeros => DiagnosticCode::LeadingZeros,
        Upstream::EmptyWordMarker => DiagnosticCode::EmptyWordMarker,
        Upstream::MissingMilestoneSelfClose => DiagnosticCode::MissingMilestoneSelfClose,
        Upstream::InvalidTableColumnSequence => DiagnosticCode::InvalidTableColumnSequence,
    }
}
