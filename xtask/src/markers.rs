//! Generates `markers.toml` from the USFM specification.
//!
//! ARCHITECTURE §6: marker semantics live in data, generated from the
//! specification and checked in. The generator is re-runnable against a later
//! specification revision, which is the point — a marker table maintained by
//! hand drifts from the standard one plausible edit at a time.
//!
//! # Where the data comes from
//!
//! The Paratext stylesheet is the specification in machine-readable form.
//! `\Marker`, `\StyleType`, `\Endmarker`, and `\OccursUnder` give class,
//! closing behaviour, and nesting for all 335 markers, which is most of what
//! the engine needs and all of what it must not get wrong.
//!
//! **`since` is derived rather than asserted.** A marker absent from the 3.0
//! stylesheet and present in the 3.1 one arrived in 3.1; that is a fact about
//! the two documents rather than a claim we are making. Where the stylesheets
//! cannot tell us — a marker present in both may date from 1.0 or 2.4 — the
//! field is left out. An absent `since` produces no diagnostic, which is the
//! right failure: silence beats a version warning we cannot substantiate.
//!
//! # What the stylesheet does not carry
//!
//! Deprecations, replacements, and attribute lists appear in the specification
//! prose and not in the stylesheet. Those live in `xtask/markers-overlay.toml`,
//! maintained by hand, and are merged in. The overlay is small and explicit so
//! that the boundary between "generated" and "asserted" stays visible.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};

use crate::corpus::{curl_to, repo_root};

/// USFM 3.1, the version the engine targets.
const STY_3_1: &str =
    "https://raw.githubusercontent.com/usfm-bible/tcdocs/main/grammar/usfm3_1.sty";
/// USFM 3.0, used only to work out which markers 3.1 introduced.
const STY_3_0: &str = "https://raw.githubusercontent.com/ubsicap/usfm/master/sty/usfm.sty";

/// Above this many parents, `OccursUnder` is recorded as `["*"]`.
///
/// Character styles legally nest under nearly every paragraph and note marker,
/// and the stylesheet enumerates all of them — roughly 200 entries each. Copied
/// out verbatim that is several megabytes of table saying "anywhere", and a
/// reviewer cannot see the exceptions for the noise. ARCHITECTURE §6 writes
/// this case as `["*"]` for the same reason.
const ANY_CONTEXT: usize = 40;

#[derive(Debug, Default, Clone)]
struct StyleRecord {
    marker: String,
    endmarker: Option<String>,
    style_type: Option<String>,
    text_type: Option<String>,
    occurs_under: Vec<String>,
    properties: Vec<String>,
}

/// Splits a Paratext stylesheet into records.
///
/// The format is line-oriented: `\Field value`, with `\Marker` opening a new
/// record and `#` starting a comment that may also trail a value.
fn parse_stylesheet(text: &str) -> Vec<StyleRecord> {
    let mut records: Vec<StyleRecord> = Vec::new();

    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim_end();
        let Some(rest) = line.strip_prefix('\\') else {
            continue;
        };
        let mut parts = rest.splitn(2, char::is_whitespace);
        let field = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("").trim();

        if field == "Marker" {
            records.push(StyleRecord {
                marker: value.to_string(),
                ..StyleRecord::default()
            });
            continue;
        }

        let Some(record) = records.last_mut() else {
            continue;
        };
        match field {
            "Endmarker" => record.endmarker = Some(value.to_string()),
            "StyleType" => record.style_type = Some(value.to_lowercase()),
            "TextType" => record.text_type = Some(value.to_lowercase()),
            "OccursUnder" => {
                record.occurs_under = value.split_whitespace().map(str::to_string).collect()
            }
            "TextProperties" => {
                record.properties = value
                    .split_whitespace()
                    .map(|property| property.to_lowercase())
                    .collect()
            }
            _ => {}
        }
    }

    records
}

/// How the marker is closed. ARCHITECTURE §6's vocabulary.
fn closing_of(record: &StyleRecord) -> &'static str {
    if is_milestone(&record.marker) {
        return "milestone";
    }
    if record.endmarker.is_some() {
        return "explicit";
    }
    match record.style_type.as_deref() {
        // A paragraph runs until the next paragraph marker closes it.
        Some("paragraph") => "implicit",
        _ => "none",
    }
}

/// Milestones are spelled `\qt-s`, `\qt1-e`, `\ts-s`.
fn is_milestone(marker: &str) -> bool {
    marker.ends_with("-s") || marker.ends_with("-e")
}

fn class_of(record: &StyleRecord) -> String {
    if is_milestone(&record.marker) {
        return "milestone".to_string();
    }
    match record.style_type.as_deref() {
        Some(style) => style.to_string(),
        // 45 of the 335 entries carry no StyleType. They are attribute and
        // structural pseudo-entries rather than markers a document writes.
        None => "unclassified".to_string(),
    }
}

// ------------------------------------------------------------- overlay ---

#[derive(Debug, Default, serde::Deserialize)]
struct Overlay {
    #[serde(default)]
    marker: BTreeMap<String, OverlayEntry>,
}

#[derive(Debug, Default, Clone, serde::Deserialize)]
struct OverlayEntry {
    since: Option<String>,
    deprecated_in: Option<String>,
    replacement: Option<String>,
    #[serde(default)]
    attributes: Vec<String>,
    default_attr: Option<String>,
}

fn load_overlay(path: &Path) -> Result<Overlay> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the overlay at {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

// ----------------------------------------------------------- generation ---

pub fn generate(offline: bool) -> Result<()> {
    let root = repo_root();
    let cache = root.join("xtask").join(".spec");
    std::fs::create_dir_all(&cache)?;

    let current = fetch(&cache, "usfm3_1.sty", STY_3_1, offline)?;
    let previous = fetch(&cache, "usfm3_0.sty", STY_3_0, offline)?;

    let records = parse_stylesheet(&current);
    let earlier: BTreeSet<String> = parse_stylesheet(&previous)
        .into_iter()
        .map(|record| record.marker)
        .collect();

    let overlay = load_overlay(&root.join("xtask").join("markers-overlay.toml"))?;

    let mut rows: BTreeMap<String, String> = BTreeMap::new();
    for record in &records {
        if record.marker.is_empty() {
            continue;
        }
        let extra = overlay
            .marker
            .get(&record.marker)
            .cloned()
            .unwrap_or_default();
        rows.insert(record.marker.clone(), render(record, &earlier, &extra));
    }

    let mut out = String::new();
    out.push_str(
        "# Generated by `cargo xtask markers generate`. Do not edit.\n\
         #\n\
         # Source: the USFM 3.1 Paratext stylesheet, which is the specification\n\
         # in machine-readable form. `since` is derived by diffing against the\n\
         # 3.0 stylesheet, and is absent where the two cannot settle it.\n\
         # Deprecations and attribute lists come from xtask/markers-overlay.toml,\n\
         # because the stylesheet does not carry them.\n\
         #\n\
         # `nests_under = [\"*\"]` means any character or note context.\n\n",
    );
    out.push_str(&format!("# {} markers\n\n", rows.len()));
    for row in rows.values() {
        out.push_str(row);
        out.push('\n');
    }

    let destination = root
        .join("crates")
        .join("easy-usfm-core")
        .join("markers.toml");
    std::fs::write(&destination, &out)?;

    let new_in_3_1 = records
        .iter()
        .filter(|record| !earlier.contains(&record.marker))
        .count();
    eprintln!(
        "{} markers written to {}\n  {} new in 3.1, {} carrying overlay metadata",
        rows.len(),
        destination.display(),
        new_in_3_1,
        overlay.marker.len(),
    );

    Ok(())
}

fn fetch(cache: &Path, name: &str, url: &str, offline: bool) -> Result<String> {
    let path = cache.join(name);
    if !path.exists() {
        anyhow::ensure!(
            !offline,
            "{} is not cached and --offline was given",
            path.display()
        );
        curl_to(url, &path)?;
    }
    std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
}

fn render(record: &StyleRecord, earlier: &BTreeSet<String>, extra: &OverlayEntry) -> String {
    let mut out = format!("[{}]\n", record.marker);
    out.push_str(&format!("class = \"{}\"\n", class_of(record)));
    out.push_str(&format!("closing = \"{}\"\n", closing_of(record)));

    let nests: Vec<String> = if record.occurs_under.len() > ANY_CONTEXT {
        vec!["*".to_string()]
    } else {
        record.occurs_under.clone()
    };
    out.push_str(&format!("nests_under = {}\n", string_list(&nests)));

    if let Some(text_type) = &record.text_type {
        out.push_str(&format!("text_type = \"{text_type}\"\n"));
    }
    out.push_str(&format!(
        "publishable = {}\n",
        record.properties.iter().any(|p| p == "publishable")
    ));

    // Derived where the stylesheets can settle it, from the overlay where a
    // human has established it, and absent otherwise.
    let since = extra
        .since
        .clone()
        .or_else(|| (!earlier.contains(&record.marker)).then(|| "3.1".to_string()));
    if let Some(since) = since {
        out.push_str(&format!("since = \"{since}\"\n"));
    }
    if let Some(deprecated) = &extra.deprecated_in {
        out.push_str(&format!("deprecated_in = \"{deprecated}\"\n"));
    }
    if let Some(replacement) = &extra.replacement {
        out.push_str(&format!("replacement = \"{replacement}\"\n"));
    }
    if !extra.attributes.is_empty() {
        out.push_str(&format!(
            "attributes = {}\n",
            string_list(&extra.attributes)
        ));
    }
    if let Some(default) = &extra.default_attr {
        out.push_str(&format!("default_attr = \"{default}\"\n"));
    }

    out
}

fn string_list(values: &[String]) -> String {
    let inner: Vec<String> = values
        .iter()
        .map(|value| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", inner.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\\Marker bd\n\
                          \\Endmarker bd*\n\
                          \\Name bd...bd* - Character - Bold Text\n\
                          \\OccursUnder p q1 q2\n\
                          \\TextProperties publishable vernacular\n\
                          \\StyleType Character\n\
                          \\FontSize 12  # trailing comment\n\
                          \n\
                          \\Marker q1\n\
                          \\OccursUnder id periph esb\n\
                          \\StyleType Paragraph\n\
                          \\TextType VerseText\n\
                          \\TextProperties paragraph publishable\n";

    #[test]
    fn records_split_on_the_marker_field() {
        let records = parse_stylesheet(SAMPLE);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].marker, "bd");
        assert_eq!(records[1].marker, "q1");
    }

    #[test]
    fn fields_are_read_and_trailing_comments_dropped() {
        let records = parse_stylesheet(SAMPLE);
        assert_eq!(records[0].endmarker.as_deref(), Some("bd*"));
        assert_eq!(records[0].style_type.as_deref(), Some("character"));
        assert_eq!(records[0].occurs_under, vec!["p", "q1", "q2"]);
        assert_eq!(records[1].text_type.as_deref(), Some("versetext"));
    }

    #[test]
    fn closing_follows_the_endmarker_and_the_style() {
        let records = parse_stylesheet(SAMPLE);
        assert_eq!(closing_of(&records[0]), "explicit");
        assert_eq!(closing_of(&records[1]), "implicit");
    }

    #[test]
    fn milestones_are_recognised_by_their_suffix() {
        let records = parse_stylesheet("\\Marker qt1-s\n\\StyleType Milestone\n");
        assert_eq!(closing_of(&records[0]), "milestone");
        assert_eq!(class_of(&records[0]), "milestone");
    }

    #[test]
    fn a_marker_absent_from_the_earlier_stylesheet_is_new() {
        let records = parse_stylesheet(SAMPLE);
        let earlier: BTreeSet<String> = ["q1".to_string()].into_iter().collect();

        assert!(render(&records[0], &earlier, &OverlayEntry::default()).contains("since = \"3.1\""));
        // Present in both, so the stylesheets cannot date it and we do not
        // guess.
        assert!(!render(&records[1], &earlier, &OverlayEntry::default()).contains("since"));
    }
}
