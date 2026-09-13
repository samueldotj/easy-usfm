//! Parser diagnostics to ours.

use crate::{Diagnostic, DiagnosticCode, Severity};

/// A study sidebar's two markers. `\esb` opens, `\esbe` closes.
const OPENER: &str = "\\esb";
const CLOSER: &str = "\\esbe";

/// Drops the pair of errors the parser raises on a *correctly closed* sidebar.
///
/// # What upstream does
///
/// `\esb … \esbe` is the one paragraph-level construct in USFM 3.1 whose
/// closing marker is a separate marker rather than a `*` form, and `usfm3`
/// does not model that. Given a properly closed block it reports both
///
/// - `UnclosedAtEof` — "\esb was still open at end of file", and
/// - `StrayCloseMarker` — "closing marker \esbe* has no matching opener"
///
/// which are not only wrong but contradict each other: the first says nothing
/// closed the block, the second points at the thing that did. A study Bible
/// carries hundreds of sidebars, so this is two errors per note across the
/// whole file — enough to bury every real diagnostic in it.
///
/// # Why here and not upstream
///
/// Both, eventually. ADR-001 pins `usfm3` at an exact version precisely so
/// that upstream's schedule is not ours, and it makes the facade the place
/// where the parser's output is corrected before anything above can see it —
/// `severity::is_derived` already drops upstream diagnostics this crate
/// re-derives better. Waiting for a release would leave the editor unusable on
/// the files the feature exists for. This belongs in the ADR's list of things
/// to raise with the author.
///
/// # How a real fault is told from this one
///
/// From the spans, which are unambiguous once you look at them:
///
/// | Document | `UnclosedAtEof` span | `StrayCloseMarker` |
/// | --- | --- | --- |
/// | properly closed | `\esb` … `\esbe` — *ends with the closer* | the `\esbe` |
/// | never closed | `\esb` … end of file | none |
/// | stray `\esbe` | none | the `\esbe` |
///
/// So the pair to drop is an `UnclosedAtEof` whose own span ends with the
/// closing marker, together with the `StrayCloseMarker` sitting at that same
/// end. Everything else is a real fault and survives — including the outer
/// block of `\esb \esb … \esbe`, whose span ends with a newline rather than
/// the closer because only the inner one was closed.
pub(super) fn without_false_sidebar_pairs(
    mut diagnostics: Vec<Diagnostic>,
    source: &str,
) -> Vec<Diagnostic> {
    let closed = |diagnostic: &Diagnostic| {
        diagnostic.code == DiagnosticCode::UnclosedAtEof
            && source
                .get(diagnostic.span.start..diagnostic.span.end)
                .is_some_and(is_closed_sidebar)
    };

    // Where each properly closed block ends, which is also where its wrongly
    // accused closing marker is. Almost every document has none, and then this
    // is one pass and no allocation beyond an empty vector.
    let ends: Vec<usize> = diagnostics
        .iter()
        .filter(|diagnostic| closed(diagnostic))
        .map(|diagnostic| diagnostic.span.end)
        .collect();

    if ends.is_empty() {
        return diagnostics;
    }

    diagnostics.retain(|diagnostic| match diagnostic.code {
        DiagnosticCode::UnclosedAtEof => !closed(diagnostic),
        // Only the one at the end of a block that turned out to be closed. A
        // `\esbe` anywhere else really has no opener.
        DiagnosticCode::StrayCloseMarker => {
            !(ends.contains(&diagnostic.span.end)
                && source.get(diagnostic.span.start..diagnostic.span.end) == Some(CLOSER))
        }
        _ => true,
    });

    diagnostics
}

/// Whether a span runs from an `\esb` to a matching `\esbe`.
fn is_closed_sidebar(text: &str) -> bool {
    // Longer than the closer on its own, so a span that *is* just an `\esbe`
    // cannot satisfy both ends of this test at once.
    text.len() > CLOSER.len()
        && text.starts_with(OPENER)
        && text.ends_with(CLOSER)
        // `\esb` and not the start of a longer name -- `\esbe` itself, or a
        // custom `\esbfoo`. Without this the opener test matches its own
        // closer.
        && text[OPENER.len()..]
            .chars()
            .next()
            .is_some_and(|next| !next.is_ascii_alphanumeric() && next != '-')
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ByteSpan;

    /// A diagnostic over the given span, which is all the rule looks at.
    fn at(code: DiagnosticCode, start: usize, end: usize) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Error,
            span: ByteSpan::new(start, end),
            message: String::new(),
        }
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    /// The spans below are the parser's real output, read off a run against
    /// each document. The boundaries are what make the rule decidable, so
    /// inventing them would be testing the rule against a guess.
    #[test]
    fn a_closed_sidebar_loses_both_of_its_false_errors() {
        let source = "\\esb\n\\p body\n\\esbe\n";
        let found = without_false_sidebar_pairs(
            vec![
                at(DiagnosticCode::UnclosedAtEof, 0, 18),
                at(DiagnosticCode::StrayCloseMarker, 13, 18),
            ],
            source,
        );

        assert!(found.is_empty(), "{:?}", codes(&found));
    }

    #[test]
    fn an_unclosed_sidebar_keeps_its_error() {
        // No closer, so the span runs to the end of the file and there is no
        // stray-close beside it.
        let source = "\\esb\n\\p body\n";
        let found =
            without_false_sidebar_pairs(vec![at(DiagnosticCode::UnclosedAtEof, 0, 13)], source);

        assert_eq!(codes(&found), ["USFM-E009"]);
    }

    #[test]
    fn a_stray_closer_keeps_its_error() {
        let source = "\\p a\n\\esbe\n";
        let found =
            without_false_sidebar_pairs(vec![at(DiagnosticCode::StrayCloseMarker, 5, 10)], source);

        assert_eq!(codes(&found), ["USFM-E004"]);
    }

    #[test]
    fn a_stray_closer_elsewhere_survives_a_closed_block() {
        // One properly closed block and, later, a closer with nothing to
        // close. Dropping by code rather than by position would silence both.
        let source = "\\esb\n\\p a\n\\esbe\n\\p x\n\\esbe\n";
        let found = without_false_sidebar_pairs(
            vec![
                at(DiagnosticCode::UnclosedAtEof, 0, 15),
                at(DiagnosticCode::StrayCloseMarker, 10, 15),
                at(DiagnosticCode::StrayCloseMarker, 21, 26),
            ],
            source,
        );

        assert_eq!(codes(&found), ["USFM-E004"]);
        assert_eq!(found[0].span.start, 21);
    }

    #[test]
    fn an_outer_block_left_open_by_a_closed_inner_one_still_reports() {
        // Two openers and one closer: the inner block is closed and the outer
        // is not. The outer span ends with a newline rather than with the
        // closer, which is exactly what tells the two apart.
        let source = "\\esb\n\\esb\n\\p a\n\\esbe\n";
        let found = without_false_sidebar_pairs(
            vec![
                at(DiagnosticCode::UnclosedAtEof, 0, 21),
                at(DiagnosticCode::UnclosedAtEof, 5, 20),
                at(DiagnosticCode::StrayCloseMarker, 15, 20),
            ],
            source,
        );

        assert_eq!(codes(&found), ["USFM-E009"]);
        assert_eq!(
            found[0].span.start, 0,
            "the outer block is the unclosed one"
        );
    }

    #[test]
    fn nothing_else_is_touched() {
        let source = "\\esb\n\\p body\n\\esbe\n";
        let found = without_false_sidebar_pairs(
            vec![
                at(DiagnosticCode::UnclosedAtEof, 0, 18),
                at(DiagnosticCode::StrayCloseMarker, 13, 18),
                at(DiagnosticCode::UnknownMarker, 5, 7),
            ],
            source,
        );

        assert_eq!(codes(&found), [DiagnosticCode::UnknownMarker.as_str()]);
    }

    #[test]
    fn a_document_with_no_sidebars_is_returned_untouched() {
        let source = "\\p a\n\\bd unclosed\n";
        let given = vec![
            at(DiagnosticCode::UnclosedAtEof, 5, 17),
            at(DiagnosticCode::StrayCloseMarker, 5, 10),
        ];
        let found = without_false_sidebar_pairs(given.clone(), source);

        assert_eq!(found.len(), given.len());
    }

    #[test]
    fn a_span_that_is_only_the_closer_is_not_read_as_a_closed_block() {
        // The closer starts with the opener and ends with itself, so without
        // the length and boundary tests the rule would match it.
        assert!(!is_closed_sidebar("\\esbe"));
        assert!(!is_closed_sidebar("\\esbfoo\n\\esbe"));
        assert!(is_closed_sidebar("\\esb\n\\esbe"));
        assert!(is_closed_sidebar("\\esb \n\\p a\n\\esbe"));
    }
}
