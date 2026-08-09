//! The marker table — what each marker is, and how it behaves.
//!
//! ARCHITECTURE §6: marker semantics live in data rather than in match arms.
//! `markers.toml` is generated from the USFM specification by
//! `cargo xtask markers generate` and checked in, so a specification revision
//! is a regenerated file to review rather than an archaeology exercise.
//!
//! The table is what makes diagnostic severity derivable rather than
//! hardcoded (PRODUCT §9): whether a marker is unknown, deprecated, or simply
//! newer than the document claims to be are all lookups here.
//!
//! # Parsed rather than deserialized
//!
//! The file is read with a small parser that accepts only what the generator
//! emits, instead of pulling in a general TOML implementation. This crate
//! compiles to wasm and ships in a worker, where a dependency is paid for on
//! every page load; a general parser for a file we generate ourselves is not
//! worth that. The parser is strict — anything unexpected is a panic at first
//! use, which for a checked-in generated file means a failing test rather
//! than a shipped bug — and `every_row_parses` walks all 335 rows.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The generated table, compiled in.
const TABLE: &str = include_str!("../markers.toml");

/// What kind of thing a marker is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkerClass {
    Character,
    Paragraph,
    Note,
    Milestone,
    /// Present in the stylesheet with no `StyleType`. These are structural and
    /// attribute pseudo-entries rather than markers a document writes.
    Unclassified,
}

/// How a marker ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Closing {
    /// Closed by `\marker*`.
    Explicit,
    /// Closed by the next paragraph marker.
    Implicit,
    /// A `-s`/`-e` pair, which may span paragraphs and verses.
    Milestone,
    /// Self-contained.
    None,
}

/// Everything the table knows about one marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerInfo {
    pub marker: &'static str,
    pub class: MarkerClass,
    pub closing: Closing,
    /// Markers this may nest under. A single `"*"` means any character or note
    /// context — character styles legally nest almost anywhere, and the
    /// stylesheet's 200-entry enumeration says nothing a reader can use.
    pub nests_under: Vec<&'static str>,
    pub text_type: Option<&'static str>,
    pub publishable: bool,
    /// The USFM version that introduced the marker, where it is known.
    ///
    /// Absent means the specification's stylesheets could not settle it, not
    /// that the marker is old. Deriving severity from an absent `since`
    /// produces no diagnostic, which is the right failure: silence beats a
    /// version warning that cannot be substantiated.
    pub since: Option<&'static str>,
    pub deprecated_in: Option<&'static str>,
    pub replacement: Option<&'static str>,
    pub attributes: Vec<&'static str>,
    pub default_attr: Option<&'static str>,
}

impl MarkerInfo {
    /// Whether this marker nests under `parent`.
    pub fn nests_under(&self, parent: &str) -> bool {
        self.nests_under
            .iter()
            .any(|allowed| *allowed == "*" || *allowed == parent)
    }

    /// Whether the marker accepts attributes at all.
    ///
    /// The link attributes are valid on every character marker, so a character
    /// style with no attributes of its own still accepts them.
    pub fn accepts_attributes(&self) -> bool {
        !self.attributes.is_empty() || self.class == MarkerClass::Character
    }
}

fn table() -> &'static BTreeMap<&'static str, MarkerInfo> {
    static TABLE_ONCE: OnceLock<BTreeMap<&'static str, MarkerInfo>> = OnceLock::new();
    TABLE_ONCE.get_or_init(|| parse(TABLE))
}

/// The marker's entry, or `None` if the specification does not define it.
///
/// Handles the two ways a marker appears in a document but not in the table:
/// the `\+` nesting prefix, and a level suffix deeper than the specification
/// enumerates. `\+bd` is `\bd` nested; `\pi5` behaves as `\pi` at level 5,
/// because USFM numbers levels open-endedly while the stylesheet stops at the
/// levels it bothered to list.
pub fn lookup(marker: &str) -> Option<&'static MarkerInfo> {
    let marker = marker.strip_prefix('+').unwrap_or(marker);

    if let Some(info) = table().get(marker) {
        return Some(info);
    }

    let base = marker.trim_end_matches(|c: char| c.is_ascii_digit());
    (base != marker).then(|| table().get(base)).flatten()
}

/// Whether the marker is in the `\z` namespace, which the specification leaves
/// open-ended by design.
pub fn is_custom(marker: &str) -> bool {
    marker.strip_prefix('+').unwrap_or(marker).starts_with('z')
}

/// Every marker the specification defines.
pub fn all() -> impl Iterator<Item = &'static MarkerInfo> {
    table().values()
}

pub fn count() -> usize {
    table().len()
}

// ---------------------------------------------------------------- parser ---

fn parse(text: &'static str) -> BTreeMap<&'static str, MarkerInfo> {
    let mut markers = BTreeMap::new();
    let mut current: Option<MarkerInfo> = None;

    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if let Some(info) = current.take() {
                markers.insert(info.marker, info);
            }
            current = Some(MarkerInfo {
                marker: name,
                class: MarkerClass::Unclassified,
                closing: Closing::None,
                nests_under: Vec::new(),
                text_type: None,
                publishable: false,
                since: None,
                deprecated_in: None,
                replacement: None,
                attributes: Vec::new(),
                default_attr: None,
            });
            continue;
        }

        let (key, value) = line
            .split_once(" = ")
            .unwrap_or_else(|| panic!("markers.toml:{}: not a field: {line}", number + 1));
        let info = current
            .as_mut()
            .unwrap_or_else(|| panic!("markers.toml:{}: field before any marker", number + 1));

        match key {
            "class" => {
                info.class = match unquote(value) {
                    "character" => MarkerClass::Character,
                    "paragraph" => MarkerClass::Paragraph,
                    "note" => MarkerClass::Note,
                    "milestone" => MarkerClass::Milestone,
                    "unclassified" => MarkerClass::Unclassified,
                    other => panic!("markers.toml:{}: unknown class {other}", number + 1),
                }
            }
            "closing" => {
                info.closing = match unquote(value) {
                    "explicit" => Closing::Explicit,
                    "implicit" => Closing::Implicit,
                    "milestone" => Closing::Milestone,
                    "none" => Closing::None,
                    other => panic!("markers.toml:{}: unknown closing {other}", number + 1),
                }
            }
            "nests_under" => info.nests_under = list(value),
            "attributes" => info.attributes = list(value),
            "text_type" => info.text_type = Some(unquote(value)),
            "since" => info.since = Some(unquote(value)),
            "deprecated_in" => info.deprecated_in = Some(unquote(value)),
            "replacement" => info.replacement = Some(unquote(value)),
            "default_attr" => info.default_attr = Some(unquote(value)),
            "publishable" => info.publishable = value == "true",
            other => panic!("markers.toml:{}: unknown field {other}", number + 1),
        }
    }

    if let Some(info) = current.take() {
        markers.insert(info.marker, info);
    }
    markers
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or_else(|| panic!("markers.toml: not a quoted string: {value}"))
}

fn list(value: &str) -> Vec<&str> {
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or_else(|| panic!("markers.toml: not a list: {value}"));

    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner.split(", ").map(unquote).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_parses() {
        // The parser panics on anything unexpected, so simply walking the
        // table is the assertion. A generated file that stopped matching the
        // parser fails here rather than in a worker.
        assert!(count() > 300, "only {} markers loaded", count());
        for info in all() {
            assert!(!info.marker.is_empty());
        }
    }

    #[test]
    fn a_character_marker_closes_explicitly_and_nests_anywhere() {
        let bd = lookup("bd").expect("\\bd is in the specification");
        assert_eq!(bd.class, MarkerClass::Character);
        assert_eq!(bd.closing, Closing::Explicit);
        assert!(bd.nests_under("p"));
        assert!(bd.nests_under("q1"));
    }

    #[test]
    fn a_paragraph_marker_closes_implicitly() {
        let q1 = lookup("q1").expect("\\q1 is in the specification");
        assert_eq!(q1.class, MarkerClass::Paragraph);
        assert_eq!(q1.closing, Closing::Implicit);
    }

    #[test]
    fn milestones_are_classified_by_their_suffix() {
        let start = lookup("qt-s").expect("\\qt-s is in the specification");
        assert_eq!(start.closing, Closing::Milestone);
        assert_eq!(start.class, MarkerClass::Milestone);
    }

    #[test]
    fn the_nesting_prefix_resolves_to_the_marker_it_nests() {
        assert_eq!(lookup("+bd").map(|i| i.marker), Some("bd"));
    }

    #[test]
    fn a_level_deeper_than_the_specification_lists_falls_back() {
        // USFM numbers levels open-endedly; the stylesheet stops where it
        // stopped. \pi5 is \pi at level 5, not an unknown marker.
        assert!(lookup("pi1").is_some());
        assert_eq!(lookup("pi9").map(|i| i.class), Some(MarkerClass::Paragraph));
    }

    #[test]
    fn deprecations_carry_their_replacement() {
        let ph = lookup("ph").expect("\\ph is in the specification");
        assert_eq!(ph.deprecated_in, Some("3.0"));
        assert_eq!(ph.replacement, Some("pi#"));

        for marker in ["addpn", "pro"] {
            assert!(
                lookup(marker).is_some_and(|i| i.deprecated_in.is_some()),
                "\\{marker} should be marked deprecated"
            );
        }
    }

    #[test]
    fn attributes_and_their_defaults_are_recorded() {
        let jmp = lookup("jmp").expect("\\jmp is in the specification");
        assert_eq!(jmp.attributes, vec!["link-href", "link-title", "link-id"]);
        assert_eq!(jmp.default_attr, Some("link-href"));

        let w = lookup("w").expect("\\w is in the specification");
        assert_eq!(w.default_attr, Some("lemma"));
    }

    #[test]
    fn markers_introduced_in_3_1_are_dated() {
        // Derived by diffing the 3.0 and 3.1 stylesheets, not asserted.
        assert_eq!(lookup("esb").and_then(|i| i.since), Some("3.1"));
        assert_eq!(lookup("cat").and_then(|i| i.since), Some("3.1"));
    }

    #[test]
    fn a_marker_the_specification_does_not_define_is_absent() {
        assert!(lookup("definitelynotamarker").is_none());
        assert!(is_custom("zaln-s"));
        assert!(!is_custom("bd"));
    }
}
