//! Script, feature, and encoding-trait detection for USFM files.
//!
//! Drives corpus selection: we want the smallest set of files covering every
//! required script, every USFM feature class, and every encoding trait, because
//! that set is committed and runs on every push.
//!
//! Marker detection is a real scan rather than a set of regexes. That costs a
//! few more lines and buys two things: no dependency, and a shape that can be
//! swapped for the marker table from P0.6 without rewriting the callers.

use std::collections::BTreeSet;
use std::fmt;

use unicode_normalization::is_nfc;
use unicode_script::{Script, UnicodeScript};

// ---------------------------------------------------------------- scripts ---

/// Scripts the committed corpus must cover, chosen to exercise combining marks,
/// conjunct formation, visual reordering, right-to-left, and the absence of
/// word spacing. See `docs/ARCHITECTURE.md` §12.4.
pub const REQUIRED_SCRIPTS: &[&str] = &[
    "Latin",
    "Greek",
    "Cyrillic",
    "Hebrew",
    "Arabic",
    "Devanagari",
    "Tamil",
    "Bengali",
    "Thai",
    "Khmer",
    "Myanmar",
    "Han",
];

/// Scripts present in `text`, ignoring any below `min_share` of scripted
/// characters. The threshold stops a stray book code or a single quoted Greek
/// word from counting as coverage.
pub fn detect_scripts(text: &str, min_share: f64) -> BTreeSet<String> {
    let mut counts: Vec<(Script, usize)> = Vec::new();
    let mut total = 0usize;

    for ch in text.chars() {
        let script = ch.script();
        // Common covers punctuation and digits; Inherited covers combining
        // marks, which take the script of what they attach to.
        if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
            continue;
        }
        total += 1;
        match counts.iter_mut().find(|(s, _)| *s == script) {
            Some((_, n)) => *n += 1,
            None => counts.push((script, 1)),
        }
    }

    if total == 0 {
        return BTreeSet::new();
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n as f64 / total as f64 >= min_share)
        .map(|(s, _)| script_name(s))
        .collect()
}

fn script_name(s: Script) -> String {
    // Debug gives the Unicode script name ("Devanagari", "Han", "Latin").
    format!("{s:?}")
}

// --------------------------------------------------------------- features ---

/// USFM construct families the parser and preview must handle. Every class must
/// appear somewhere in the committed corpus, so a change to note handling cannot
/// pass CI without a file that has notes.
pub const FEATURE_CLASSES: &[&str] = &[
    "notes",
    "poetry",
    "lists",
    "tables",
    "milestones",
    "attributes",
    "sidebars",
    "figures",
    "introductions",
    "peripherals",
    "custom_z",
    "titles",
    "char_styles",
    "alt_numbering",
    "verse_ranges",
    "nested_markers",
];

/// One marker occurrence: `\+bd`, `\q1`, `\qt-s`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// Tag without the backslash, level, nesting prefix, or milestone suffix.
    pub tag: String,
    /// Trailing digits, as in `q1` or `li2`.
    pub level: Option<u8>,
    /// `\+bd` — nested inside another character marker or a note.
    pub nested: bool,
    /// `-s` or `-e` on a milestone.
    pub milestone: Option<char>,
    /// Closing form, as in `\bd*`.
    pub closing: bool,
}

/// Scan every marker in `text`. Tolerant by design: this runs over files that
/// have not been validated, so anything unrecognised is simply not a marker.
pub fn scan_markers(text: &str) -> Vec<Marker> {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != '\\' {
            i += 1;
            continue;
        }
        i += 1;
        let nested = i < bytes.len() && bytes[i] == '+';
        if nested {
            i += 1;
        }

        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        if i == start {
            continue; // `\` not followed by a tag — `\*`, or literal text
        }
        let tag: String = bytes[start..i].iter().collect();

        let lvl_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let level: Option<u8> = if i > lvl_start {
            bytes[lvl_start..i].iter().collect::<String>().parse().ok()
        } else {
            None
        };

        let mut milestone = None;
        if i + 1 < bytes.len() && bytes[i] == '-' && matches!(bytes[i + 1], 's' | 'e') {
            milestone = Some(bytes[i + 1]);
            i += 2;
        }

        let closing = i < bytes.len() && bytes[i] == '*';
        if closing {
            i += 1;
        }

        out.push(Marker {
            tag,
            level,
            nested,
            milestone,
            closing,
        });
    }
    out
}

/// Feature classes exercised by `text`.
pub fn detect_features(text: &str) -> BTreeSet<String> {
    let markers = scan_markers(text);
    let mut found = BTreeSet::new();

    for m in &markers {
        if m.nested {
            found.insert("nested_markers".to_string());
        }
        if m.milestone.is_some() && !m.tag.is_empty() {
            found.insert("milestones".to_string());
        }
        let class = match m.tag.as_str() {
            "f" | "fe" | "ef" | "efe" | "x" | "ex" => Some("notes"),
            "q" | "qr" | "qc" | "qa" | "qm" | "qd" => Some("poetry"),
            "li" | "lh" | "lf" | "lim" => Some("lists"),
            "tr" | "th" | "thr" | "thc" | "tc" | "tcr" | "tcc" => Some("tables"),
            "esb" | "esbe" => Some("sidebars"),
            "fig" => Some("figures"),
            "imt" | "is" | "ip" | "ipi" | "im" | "imi" | "ipq" | "imq" | "ipr" | "ipc" | "iq"
            | "ili" | "ib" | "iot" | "io" | "iex" | "ie" => Some("introductions"),
            "periph" => Some("peripherals"),
            "mt" | "mte" | "ms" | "mr" | "s" | "sr" | "r" | "d" | "sp" | "sd" | "cl" | "cd" => {
                Some("titles")
            }
            "bd" | "it" | "bdit" | "em" | "sc" | "no" | "nd" | "wj" | "add" | "k" | "w" | "tl"
            | "pn" | "png" | "qt" | "sig" | "sls" | "bk" | "ord" | "rb" | "rq" | "ref" => {
                Some("char_styles")
            }
            "va" | "vp" | "ca" | "cp" => Some("alt_numbering"),
            t if t.starts_with('z') && t.len() > 1 => Some("custom_z"),
            _ => None,
        };
        if let Some(c) = class {
            found.insert(c.to_string());
        }
    }

    if has_attribute(text) {
        found.insert("attributes".to_string());
    }
    if has_verse_range(text) {
        found.insert("verse_ranges".to_string());
    }
    found
}

/// `|lemma="grace"` or the default-attribute shorthand followed by `key=`.
fn has_attribute(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c != '|' {
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '-') {
            j += 1;
        }
        while j < chars.len() && chars[j] == ' ' {
            j += 1;
        }
        if j > i + 1 && j < chars.len() && chars[j] == '=' {
            return true;
        }
    }
    false
}

/// `\v 1-2` — a bridged verse. Also accepts an en dash, which occurs in the wild.
fn has_verse_range(text: &str) -> bool {
    for line in text.lines() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("\\v ") else {
            continue;
        };
        let rest = rest.trim_start();
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            continue;
        }
        let after = &rest[digits.len()..];
        if after.starts_with('-') || after.starts_with('\u{2013}') {
            let tail = &after[after.chars().next().unwrap().len_utf8()..];
            if tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

// ----------------------------------------------------------------- traits ---

/// Byte-level traits relevant to the fidelity guarantees in FILE-FIDELITY.md.
pub const TRAIT_CLASSES: &[&str] = &[
    "bom",
    "lf",
    "crlf",
    "mixed_eol",
    "no_final_newline",
    "not_nfc",
    "joiners",
];

pub fn detect_traits(raw: &[u8]) -> BTreeSet<String> {
    let mut traits = BTreeSet::new();
    let bom = raw.starts_with(&[0xEF, 0xBB, 0xBF]);
    if bom {
        traits.insert("bom".to_string());
    }
    let body = if bom { &raw[3..] } else { raw };

    let crlf = count_sub(body, b"\r\n");
    let lf = body.iter().filter(|b| **b == b'\n').count() - crlf;
    let cr = body.iter().filter(|b| **b == b'\r').count() - crlf;

    if crlf > 0 && (lf > 0 || cr > 0) {
        traits.insert("mixed_eol".to_string());
    } else if crlf > 0 {
        traits.insert("crlf".to_string());
    } else if cr > 0 {
        traits.insert("cr".to_string());
    } else {
        traits.insert("lf".to_string());
    }

    if !body.is_empty() && !matches!(body.last(), Some(b'\n') | Some(b'\r')) {
        traits.insert("no_final_newline".to_string());
    }

    match std::str::from_utf8(body) {
        Err(_) => {
            traits.insert("invalid_utf8".to_string());
        }
        Ok(text) => {
            if !is_nfc(text) {
                traits.insert("not_nfc".to_string());
            }
            if text.contains('\u{200c}') || text.contains('\u{200d}') {
                traits.insert("joiners".to_string());
            }
        }
    }
    traits
}

fn count_sub(hay: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || hay.len() < needle.len() {
        return 0;
    }
    let mut n = 0;
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            n += 1;
            i += needle.len();
        } else {
            i += 1;
        }
    }
    n
}

// ------------------------------------------------------------------ input ---

/// Decode a USFM file for analysis, dropping a BOM and tolerating bad bytes.
pub fn read_text(raw: &[u8]) -> String {
    let body = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &raw[3..]
    } else {
        raw
    };
    String::from_utf8_lossy(body).into_owned()
}

/// What a file contributes, as opposed to everything it happens to contain.
pub struct Profile {
    pub scripts: BTreeSet<String>,
    pub features: BTreeSet<String>,
    pub traits: BTreeSet<String>,
}

impl Profile {
    pub fn of(raw: &[u8]) -> Self {
        let text = read_text(raw);
        Profile {
            scripts: detect_scripts(&text, 0.01),
            features: detect_features(&text),
            traits: detect_traits(raw),
        }
    }

    /// Coverage goals this file satisfies.
    pub fn goals(&self) -> BTreeSet<String> {
        let mut g = BTreeSet::new();
        for s in &self.scripts {
            if REQUIRED_SCRIPTS.contains(&s.as_str()) {
                g.insert(s.clone());
            }
        }
        g.extend(self.features.iter().cloned());
        for t in &self.traits {
            if TRAIT_CLASSES.contains(&t.as_str()) {
                g.insert(t.clone());
            }
        }
        g
    }
}

pub fn all_goals() -> BTreeSet<String> {
    REQUIRED_SCRIPTS
        .iter()
        .chain(FEATURE_CLASSES.iter())
        .chain(TRAIT_CLASSES.iter())
        .map(|s| s.to_string())
        .collect()
}

/// Comma-joined, or `-` when empty. Used in table output.
pub struct Joined<'a>(pub &'a BTreeSet<String>);

impl fmt::Display for Joined<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return write!(f, "-");
        }
        let v: Vec<&str> = self.0.iter().map(|s| s.as_str()).collect();
        write!(f, "{}", v.join(","))
    }
}
