//! Zero-width joiners, and where they must not be.
//!
//! UNICODE's appendix: "U+200D and U+200C control conjunct formation in
//! several scripts. They change rendering, carry meaning, are invisible, and
//! are trivially inserted or deleted by accident."
//!
//! That last clause is the whole problem. A joiner that drifts into a marker
//! name produces an unknown-marker error whose cause cannot be seen — the file
//! looks correct on screen and the diagnostic looks wrong. Reporting the
//! joiner itself is the difference between a five-minute fix and an afternoon.
//!
//! # Why this is a scan and not a tree walk
//!
//! The parser has already lost the distinction by the time it has a tree: a
//! marker with a joiner in it is simply an unknown marker, and the joiner is
//! inside the name. The question is about the *characters*, so it is asked of
//! the source.

use crate::{ByteSpan, Diagnostic, DiagnosticCode, Severity};

/// Zero-width non-joiner and zero-width joiner.
const ZWNJ: char = '\u{200c}';
const ZWJ: char = '\u{200d}';

pub fn is_joiner(character: char) -> bool {
    character == ZWNJ || character == ZWJ
}

/// The name of a joiner, for a message that can be acted on.
///
/// "U+200D" is what a translator can search for and what a colleague can be
/// told over the phone; "an invisible character" is not.
fn name(character: char) -> &'static str {
    if character == ZWJ {
        "U+200D zero-width joiner"
    } else {
        "U+200C zero-width non-joiner"
    }
}

/// Whether a character can appear in a marker name.
fn in_marker_name(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-' || character == '+' || character == '*'
}

/// Every joiner that is somewhere it should not be.
///
/// Two conditions, and the distinction between them is the point:
///
/// - **`USFM-W022`**, a joiner *inside* a marker name. Almost certainly
///   accidental, and it turns a valid marker into an unknown one.
/// - **`USFM-I023`**, a joiner *adjacent* to a marker boundary — immediately
///   before the backslash or immediately after the name ends. Legal, and worth
///   mentioning because it is the position a joiner reaches by being typed one
///   character away from where it was meant.
///
/// Joiners in ordinary text are not reported at all. They belong there; that
/// is what they are for.
pub fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut found = Vec::new();

    for (offset, character) in source.char_indices() {
        if !is_joiner(character) {
            continue;
        }

        let span = ByteSpan::new(offset, offset + character.len_utf8());
        let after = source[offset + character.len_utf8()..].chars().next();

        // Whether walking back over name characters reaches a backslash. That
        // is what makes this position part of a marker rather than of text.
        let in_marker = opens_marker(source, offset);

        // Inside the *name* needs a name character on the other side too. A
        // joiner is not a legal name character, so it ends the name: a marker
        // written as b, d, joiner is the marker itself followed by a joiner,
        // while b, joiner, d is a name with one inside it. Getting this
        // backwards reports every trailing joiner as a broken marker.
        let inside_name = in_marker && after.is_some_and(in_marker_name);

        if inside_name {
            found.push(Diagnostic {
                code: DiagnosticCode::JoinerInMarkerName,
                severity: Severity::Warning,
                span,
                message: format!(
                    "{} is inside a marker name, which makes it a marker USFM does not define",
                    name(character)
                ),
            });
            continue;
        }

        // Against a boundary: just past a marker name, or just before the
        // backslash that starts one. Legal, and worth mentioning because it is
        // one keystroke from where it was probably meant to go.
        if in_marker || after == Some('\\') {
            found.push(Diagnostic {
                code: DiagnosticCode::JoinerAtMarkerBoundary,
                severity: Severity::Information,
                span,
                message: format!(
                    "{} sits against a marker boundary; check it is where you meant",
                    name(character)
                ),
            });
        }
    }

    found
}

/// Whether the run of name characters ending at `offset` is opened by a
/// backslash.
///
/// Distinguishes a marker followed by a joiner from ordinary text followed by
/// one, which is nobody's business.
fn opens_marker(source: &str, offset: usize) -> bool {
    let mut back = offset;
    while back > 0 {
        match source[..back].chars().next_back() {
            Some(c) if in_marker_name(c) => back -= c.len_utf8(),
            _ => break,
        }
    }
    back > 0 && source.as_bytes()[back - 1] == b'\\'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(source: &str) -> Vec<DiagnosticCode> {
        diagnostics(source).into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_joiner_inside_a_marker_name_is_a_warning() {
        // The case the diagnostic exists for: the file looks correct and the
        // unknown-marker error looks wrong.
        assert_eq!(
            codes("\\b\u{200d}d text\n"),
            vec![DiagnosticCode::JoinerInMarkerName]
        );
    }

    #[test]
    fn a_joiner_straight_after_a_backslash_is_the_same_mistake() {
        assert_eq!(
            codes("\\\u{200d}p text\n"),
            vec![DiagnosticCode::JoinerInMarkerName]
        );
    }

    #[test]
    fn a_joiner_against_a_marker_boundary_is_information() {
        // Legal, and worth mentioning: it is one keystroke from where it was
        // probably meant to go.
        assert_eq!(
            codes("\\p text\u{200d}\\bd bold\\bd*\n"),
            vec![DiagnosticCode::JoinerAtMarkerBoundary]
        );
        assert_eq!(
            codes("\\bd\u{200d} bold\n"),
            vec![DiagnosticCode::JoinerAtMarkerBoundary]
        );
    }

    #[test]
    fn a_joiner_in_ordinary_text_is_not_reported() {
        // It belongs there. That is what joiners are for, and a warning on
        // every one would make the panel useless in the scripts that need
        // them most.
        assert!(codes("\\p \u{915}\u{94d}\u{200d}\u{937} text\n").is_empty());
        assert!(codes("\\v 1 word\u{200c}word more\n").is_empty());
    }

    #[test]
    fn both_joiners_are_recognised_and_named() {
        for joiner in ['\u{200c}', '\u{200d}'] {
            let source = format!("\\b{joiner}d text\n");
            let found = diagnostics(&source);
            assert_eq!(found.len(), 1, "{joiner:?}");
            // The message names the codepoint, which is what someone can
            // search for and tell a colleague over the phone.
            assert!(found[0].message.contains("U+200"), "{}", found[0].message);
        }
    }

    #[test]
    fn the_span_covers_the_joiner_and_nothing_else() {
        let source = "\\b\u{200d}d text\n";
        let found = diagnostics(source);
        assert_eq!(found[0].span.slice(source), Some("\u{200d}"));
    }

    #[test]
    fn a_clean_document_reports_nothing() {
        assert!(diagnostics("\\id GEN\n\\c 1\n\\p\n\\v 1 plain text\n").is_empty());
    }
}
