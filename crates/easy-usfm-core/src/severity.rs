//! Marker diagnostics, with severity derived rather than hardcoded.
//!
//! PRODUCT §9 states severity as a table over the marker data, not as a list
//! of rules:
//!
//! | Condition | Severity |
//! |---|---|
//! | Structurally invalid at any version | Error |
//! | Marker absent from the table and not `\z…` | Warning |
//! | Marker `deprecated_in` ≤ target version | Warning |
//! | Marker `since` > detected document version | Information |
//! | Unknown `\z…` marker | Information |
//! | 2.x positional `\fig` syntax | Warning + quick fix |
//!
//! The last four are computed here, from `markers.toml`. They are computed
//! rather than taken from the parser because the parser has no version model:
//! it reports an unknown marker as an error at any version, which is right for
//! a validator and wrong for an editor, where the same file is legitimately
//! older or newer than the construct being flagged.
//!
//! The first row stays with the parser. Structural invalidity does not depend
//! on a version, so there is nothing to derive.

use std::collections::BTreeSet;

use crate::markers::{self, MarkerClass};
use crate::{ByteSpan, Diagnostic, DiagnosticCode, Node, Severity, Version};

/// How diagnostics are to be judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticConfig {
    /// The version being authored for. Deprecation is measured against this.
    pub target_version: Version,
    /// The version the document declares, or the assumed one. Novelty is
    /// measured against this.
    pub document_version: Version,
    /// Codes the user has chosen not to see.
    ///
    /// By code rather than by message, which is why the codes are stable: a
    /// suppression must survive a wording change.
    pub suppressed: BTreeSet<DiagnosticCode>,
}

impl DiagnosticConfig {
    /// Judged against the engine's target, with the document's own declared
    /// version.
    pub fn for_source(source: &str) -> Self {
        Self {
            target_version: Version::TARGET,
            document_version: Version::of(source),
            suppressed: BTreeSet::new(),
        }
    }

    pub fn suppress(mut self, code: DiagnosticCode) -> Self {
        self.suppressed.insert(code);
        self
    }

    pub fn is_suppressed(&self, code: DiagnosticCode) -> bool {
        self.suppressed.contains(&code)
    }
}

impl Default for DiagnosticConfig {
    fn default() -> Self {
        Self {
            target_version: Version::TARGET,
            document_version: Version::ASSUMED,
            suppressed: BTreeSet::new(),
        }
    }
}

/// Codes this module owns, and which the parser's own versions of are
/// therefore discarded.
///
/// The parser reports both without a version model. Keeping its copies would
/// mean two diagnostics on the same marker disagreeing about how much it
/// matters.
pub(crate) const DERIVED: &[DiagnosticCode] = &[
    DiagnosticCode::UnknownMarker,
    DiagnosticCode::DeprecatedMarker,
];

pub(crate) fn is_derived(code: DiagnosticCode) -> bool {
    DERIVED.contains(&code)
}

/// Walks the tree and reports what the marker table knows.
pub(crate) fn marker_diagnostics(
    nodes: &[Node],
    source: &str,
    config: &DiagnosticConfig,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for node in nodes {
        for node in node.descendants() {
            inspect(node, source, config, &mut diagnostics);
        }
    }
    diagnostics
}

fn inspect(node: &Node, source: &str, config: &DiagnosticConfig, into: &mut Vec<Diagnostic>) {
    let Some(marker) = node.marker.as_ref() else {
        return;
    };
    // Without a span there is nowhere to put the diagnostic, and a diagnostic
    // with a fabricated location is worse than none.
    let Some(span) = node.span.clone() else {
        return;
    };
    let name = marker.as_str();

    match markers::lookup(name) {
        None if markers::is_custom(name) => into.push(Diagnostic {
            code: DiagnosticCode::UnknownMarker,
            // The `\z` namespace is open by design, so an unrecognised one is
            // not a defect. unfoldingWord's alignment data is built from these.
            severity: Severity::Information,
            span,
            message: format!("\\{name} is a custom marker; its meaning is not defined by USFM"),
        }),

        None => into.push(Diagnostic {
            code: DiagnosticCode::UnknownMarker,
            severity: Severity::Warning,
            span,
            message: format!("\\{name} is not a USFM marker"),
        }),

        Some(info) => {
            if let Some(deprecated) = info.deprecated_in.and_then(Version::parse) {
                if deprecated <= config.target_version {
                    let replacement = info
                        .replacement
                        .map(|r| format!("; use \\{r}"))
                        .unwrap_or_default();
                    into.push(Diagnostic {
                        code: DiagnosticCode::DeprecatedMarker,
                        severity: Severity::Warning,
                        span: span.clone(),
                        message: format!(
                            "\\{name} was deprecated in USFM {deprecated}{replacement}"
                        ),
                    });
                }
            }

            if let Some(since) = info.since.and_then(Version::parse) {
                if since > config.document_version {
                    into.push(Diagnostic {
                        code: DiagnosticCode::MarkerNewerThanDocument,
                        severity: Severity::Information,
                        span: span.clone(),
                        message: format!(
                            "\\{name} was introduced in USFM {since}, but this document \
                             declares {}",
                            config.document_version
                        ),
                    });
                }
            }

            if info.class == MarkerClass::Unclassified {
                return;
            }
            if name == "fig" {
                if let Some(diagnostic) = legacy_figure(node, source, &span) {
                    into.push(diagnostic);
                }
            }
        }
    }
}

/// The USFM 2.x positional `\fig` form.
///
/// 2.x wrote `\fig caption|file|size|loc|copy|ref\fig*` — six fields separated
/// by `|` with no names. 3.x names them. They are told apart by whether the
/// text after the first `|` contains an `=`, which is the same test a reader
/// applies.
///
/// Detected from the source rather than from the parsed attributes, because
/// the parser does not reject the old form — it treats the whole positional
/// tail as the default attribute, yielding `src="image.png|span|||1.1"`. That
/// is a silent mis-parse of a real construct in real 2.x files, and it is the
/// reason PRODUCT §9 asks for this diagnostic to carry a quick fix rather
/// than just a warning.
fn legacy_figure(_node: &Node, source: &str, span: &ByteSpan) -> Option<Diagnostic> {
    let text = span.slice(source)?;
    let (_, tail) = text.split_once('|')?;
    if tail.contains('=') {
        return None;
    }

    Some(Diagnostic {
        code: DiagnosticCode::LegacyFigureSyntax,
        severity: Severity::Warning,
        span: span.clone(),
        message: "\\fig uses the USFM 2.x positional form; name the attributes \
                  (src, size, ref)"
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    fn diagnostics_of(source: &str, config: DiagnosticConfig) -> Vec<Diagnostic> {
        let document = Document::parse_with(source.to_string(), config);
        document.diagnostics().to_vec()
    }

    fn has(diagnostics: &[Diagnostic], code: DiagnosticCode, severity: Severity) -> bool {
        diagnostics
            .iter()
            .any(|d| d.code == code && d.severity == severity)
    }

    #[test]
    fn an_unknown_marker_is_a_warning() {
        let found = diagnostics_of(
            "\\id GEN\n\\c 1\n\\p\n\\v 1 \\notamarker text\n",
            DiagnosticConfig::default(),
        );
        assert!(has(
            &found,
            DiagnosticCode::UnknownMarker,
            Severity::Warning
        ));
    }

    #[test]
    fn an_unknown_custom_marker_is_only_information() {
        // The \z namespace is open by design. Treating it as a warning would
        // put one on every line of an alignment-bearing file.
        let found = diagnostics_of(
            "\\id GEN\n\\c 1\n\\p\n\\v 1 \\zwhatever text\n",
            DiagnosticConfig::default(),
        );
        assert!(has(
            &found,
            DiagnosticCode::UnknownMarker,
            Severity::Information
        ));
        assert!(!has(
            &found,
            DiagnosticCode::UnknownMarker,
            Severity::Warning
        ));
    }

    #[test]
    fn a_deprecated_marker_is_a_warning_and_names_its_replacement() {
        let found = diagnostics_of("\\id GEN\n\\c 1\n\\ph1 text\n", DiagnosticConfig::default());
        let deprecated = found
            .iter()
            .find(|d| d.code == DiagnosticCode::DeprecatedMarker)
            .expect("\\ph1 is deprecated");

        assert_eq!(deprecated.severity, Severity::Warning);
        assert!(deprecated.message.contains("pi1"), "{}", deprecated.message);
    }

    #[test]
    fn deprecation_is_measured_against_the_target_version() {
        // Authoring for 2.0, \ph1 is not yet deprecated.
        let config = DiagnosticConfig {
            target_version: Version::V2_0,
            ..DiagnosticConfig::default()
        };
        let found = diagnostics_of("\\id GEN\n\\c 1\n\\ph1 text\n", config);
        assert!(!found
            .iter()
            .any(|d| d.code == DiagnosticCode::DeprecatedMarker));
    }

    #[test]
    fn a_marker_newer_than_the_document_is_information() {
        // \esb arrived in 3.1; the document says 3.0.
        let found = diagnostics_of(
            "\\id GEN\n\\usfm 3.0\n\\c 1\n\\p\n\\esb\n\\p body\n\\esbe\n",
            DiagnosticConfig::for_source("\\id GEN\n\\usfm 3.0\n"),
        );
        assert!(has(
            &found,
            DiagnosticCode::MarkerNewerThanDocument,
            Severity::Information
        ));
    }

    #[test]
    fn a_marker_is_not_newer_than_a_document_that_declares_its_version() {
        let source = "\\id GEN\n\\usfm 3.1\n\\c 1\n\\p\n\\esb\n\\p body\n\\esbe\n";
        let found = diagnostics_of(source, DiagnosticConfig::for_source(source));
        assert!(!found
            .iter()
            .any(|d| d.code == DiagnosticCode::MarkerNewerThanDocument));
    }

    #[test]
    fn the_2_x_positional_figure_form_is_a_warning() {
        let found = diagnostics_of(
            "\\id GEN\n\\c 1\n\\p\n\\v 1 \\fig caption|image.png|span|||1.1\\fig*\n",
            DiagnosticConfig::default(),
        );
        assert!(has(
            &found,
            DiagnosticCode::LegacyFigureSyntax,
            Severity::Warning
        ));
    }

    #[test]
    fn the_3_x_named_figure_form_is_not_flagged() {
        let found = diagnostics_of(
            "\\id GEN\n\\c 1\n\\p\n\\v 1 \\fig caption|src=\"i.png\" size=\"span\"\\fig*\n",
            DiagnosticConfig::default(),
        );
        assert!(!found
            .iter()
            .any(|d| d.code == DiagnosticCode::LegacyFigureSyntax));
    }

    #[test]
    fn suppression_is_by_code() {
        let source = "\\id GEN\n\\c 1\n\\p\n\\v 1 \\notamarker text\n";
        let config = DiagnosticConfig::default().suppress(DiagnosticCode::UnknownMarker);

        let found = diagnostics_of(source, config);
        assert!(!found
            .iter()
            .any(|d| d.code == DiagnosticCode::UnknownMarker));
    }

    #[test]
    fn every_marker_diagnostic_points_at_sliceable_source() {
        let source = "\\id GEN\n\\c 1\n\\p\n\\v 1 \\notamarker க்ஷ \\zfoo x\n\\ph1 y\n";
        for diagnostic in diagnostics_of(source, DiagnosticConfig::default()) {
            assert!(
                diagnostic.span.slice(source).is_some(),
                "{} points at {:?}",
                diagnostic.code,
                diagnostic.span
            );
        }
    }
}
