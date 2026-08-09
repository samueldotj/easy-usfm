//! Marker autocomplete — what to offer when someone types `\`.
//!
//! PRODUCT §6: "Typing `\` opens the marker list, ranked by validity in
//! context, then frequency in the document, then alphabetically. Deprecated
//! markers are greyed with their replacement shown and never ranked first."
//!
//! # Why validity is a rank and not a filter
//!
//! Because the context can be wrong. A file mid-edit has unclosed markers, and
//! the position the parser thinks you are in is the position the *broken* file
//! puts you in — offering only what is valid there would hide `\p` from
//! someone whose document is temporarily inside a `\bd` they are about to
//! close. The list stays complete and puts the plausible things first, which
//! is the behaviour that survives being wrong.

use std::collections::BTreeMap;

use crate::markers::{self, Closing, MarkerClass, MarkerInfo};

/// Where the `\` was typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    /// Nothing but whitespace precedes the backslash on its line.
    ///
    /// The one structural fact USFM turns on: paragraph markers are
    /// line-initial and character markers are not.
    pub line_initial: bool,
    /// The innermost character or note marker still open here, if any.
    pub inside: Option<String>,
}

/// One offer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Completion {
    pub marker: &'static str,
    pub class: MarkerClass,
    /// Whether this marker makes sense where the cursor is.
    pub valid_here: bool,
    /// Set when the marker is deprecated, so the interface can grey it.
    pub deprecated_in: Option<&'static str>,
    pub replacement: Option<&'static str>,
    /// How many times the document already uses it.
    pub uses: u32,
    /// The text to put after the backslash.
    pub insert: String,
    /// Where the caret goes within `insert`, in UTF-16 units — between the
    /// marker and its closing, where the text is going to be typed.
    pub caret: u32,
    /// A short description for the list.
    pub detail: String,
}

/// Whether a marker can legally appear at `context`.
///
/// Deliberately coarse. The stylesheet's nesting data is a 200-entry
/// enumeration that reduces to "character styles nest almost anywhere", so
/// asking it precisely would produce a confident answer with no more
/// information in it than this one.
fn valid_here(info: &MarkerInfo, context: &Context) -> bool {
    match info.class {
        // Never offered: these are structural and attribute pseudo-entries in
        // the stylesheet, not markers a document writes.
        MarkerClass::Unclassified => false,

        // Line-initial by definition. Offering `\p` in the middle of a line is
        // offering to break the paragraph the cursor is in.
        MarkerClass::Paragraph => context.line_initial && context.inside.is_none(),

        // Inside another character marker, nesting has to be legal.
        MarkerClass::Character => match &context.inside {
            Some(parent) => info.nests_under(parent),
            None => true,
        },

        // A note may not open inside another note, and a milestone may appear
        // anywhere -- that is what milestones are for.
        MarkerClass::Note => context.inside.is_none(),
        MarkerClass::Milestone => true,
    }
}

/// What typing this marker should produce, and where the caret lands.
///
/// The closing marker is inserted with the opening one. PRODUCT §6.1 has
/// formatting emit real USFM rather than hidden state, and the same argument
/// applies here: the pair is what the marker *is*, and an unclosed `\bd` is
/// the single most common defect in hand-edited files.
fn insertion(info: &MarkerInfo, context: &Context) -> (String, u32) {
    // Inside another character marker, USFM requires the `+` prefix. Getting
    // this wrong is `USFM-W014`, and it is exactly the kind of rule nobody
    // should have to remember while typing.
    let prefix = match (&context.inside, info.class) {
        (Some(_), MarkerClass::Character) => "+",
        _ => "",
    };
    let name = format!("{prefix}{}", info.marker);

    match info.closing {
        Closing::Explicit => {
            let text = format!("{name} \\{name}*");
            // Just past the space, where the content goes.
            (text, (name.chars().count() + 1) as u32)
        }
        Closing::Milestone => {
            let text = format!("{name}\\*");
            (text, name.chars().count() as u32)
        }
        Closing::Implicit | Closing::None => {
            let text = format!("{name} ");
            let caret = text.chars().count() as u32;
            (text, caret)
        }
    }
}

fn detail(info: &MarkerInfo) -> String {
    let class = match info.class {
        MarkerClass::Character => "character",
        MarkerClass::Paragraph => "paragraph",
        MarkerClass::Note => "note",
        MarkerClass::Milestone => "milestone",
        MarkerClass::Unclassified => "other",
    };

    match (info.deprecated_in, info.replacement) {
        (Some(version), Some(replacement)) => {
            format!("{class} · deprecated in {version}, use \\{replacement}")
        }
        (Some(version), None) => format!("{class} · deprecated in {version}"),
        _ => class.to_string(),
    }
}

/// Counts how often each marker already appears.
///
/// A scan rather than a parse. This is a histogram, not an interpretation —
/// being slightly generous about what counts as a marker changes the order of
/// two suggestions and nothing else, and the parse it would otherwise force on
/// every `\` costs far more than that is worth.
pub fn frequencies(source: &str) -> BTreeMap<&str, u32> {
    let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
    let bytes = source.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }

        let start = index + 1;
        let mut end = start;
        // The `+` is the nesting prefix, and `\bd` and `\+bd` are the same
        // marker for the purpose of "how often do you use this".
        if end < bytes.len() && bytes[end] == b'+' {
            end += 1;
        }
        let name_start = end;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
            end += 1;
        }

        if end > name_start {
            *counts.entry(&source[name_start..end]).or_default() += 1;
        }
        // Past the name, and past a closing `*` if there is one.
        index = end.max(start);
    }

    counts
}

/// Every marker, ranked for this context.
///
/// The whole table every time, filtered by the interface as the user types.
/// 335 entries is small enough that ranking them all costs less than deciding
/// which ones to leave out, and a list that silently omits the marker someone
/// is looking for is worse than a long one.
pub fn completions(context: &Context, source: &str) -> Vec<Completion> {
    let counts = frequencies(source);

    let mut offers: Vec<Completion> = markers::all()
        .filter(|info| info.class != MarkerClass::Unclassified)
        .map(|info| {
            let (insert, caret) = insertion(info, context);
            Completion {
                marker: info.marker,
                class: info.class,
                valid_here: valid_here(info, context),
                deprecated_in: info.deprecated_in,
                replacement: info.replacement,
                uses: counts.get(info.marker).copied().unwrap_or(0),
                insert,
                caret,
                detail: detail(info),
            }
        })
        .collect();

    offers.sort_by(|a, b| {
        // PRODUCT §6: validity, then frequency, then alphabetically — with
        // deprecation slotted in second so a deprecated marker can never
        // outrank a current one, whatever the counts say. The document that
        // uses `\ph` two hundred times is exactly the document that must not
        // be offered it first.
        b.valid_here
            .cmp(&a.valid_here)
            .then(a.deprecated_in.is_some().cmp(&b.deprecated_in.is_some()))
            .then(b.uses.cmp(&a.uses))
            .then(a.marker.cmp(b.marker))
    });

    offers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_line_start() -> Context {
        Context {
            line_initial: true,
            inside: None,
        }
    }

    fn mid_line() -> Context {
        Context {
            line_initial: false,
            inside: None,
        }
    }

    fn rank(offers: &[Completion], marker: &str) -> usize {
        offers
            .iter()
            .position(|offer| offer.marker == marker)
            .unwrap_or_else(|| panic!("\\{marker} was not offered at all"))
    }

    #[test]
    fn a_paragraph_marker_is_valid_at_the_start_of_a_line_and_not_within_one() {
        let start = completions(&at_line_start(), "");
        let middle = completions(&mid_line(), "");

        assert!(start[rank(&start, "p")].valid_here);
        assert!(!middle[rank(&middle, "p")].valid_here);
    }

    #[test]
    fn a_character_marker_is_valid_in_both() {
        let start = completions(&at_line_start(), "");
        let middle = completions(&mid_line(), "");

        assert!(start[rank(&start, "bd")].valid_here);
        assert!(middle[rank(&middle, "bd")].valid_here);
    }

    #[test]
    fn what_is_valid_here_comes_first() {
        // The criterion: valid-in-context first.
        let offers = completions(&mid_line(), "");
        let first_invalid = offers
            .iter()
            .position(|offer| !offer.valid_here)
            .expect("some marker is invalid mid-line");
        let last_valid = offers
            .iter()
            .rposition(|offer| offer.valid_here)
            .expect("some marker is valid mid-line");

        assert!(last_valid < first_invalid, "the two groups are interleaved");
    }

    #[test]
    fn a_deprecated_marker_is_never_ranked_first() {
        // Not even when the document is full of it. `\ph` is deprecated in
        // favour of `\pi#`, and a file using it two hundred times is exactly
        // the file that must not be offered it first.
        let source = "\\ph text\n".repeat(200);
        let offers = completions(&at_line_start(), &source);

        assert!(offers[0].deprecated_in.is_none(), "{:?}", offers[0]);
        assert!(offers[rank(&offers, "ph")].uses >= 200);
    }

    #[test]
    fn a_deprecated_marker_carries_its_replacement_for_the_grey_text() {
        let offers = completions(&at_line_start(), "");
        let ph = &offers[rank(&offers, "ph")];

        assert_eq!(ph.deprecated_in, Some("3.0"));
        assert_eq!(ph.replacement, Some("pi#"));
        assert!(ph.detail.contains("pi#"), "{}", ph.detail);
    }

    #[test]
    fn frequency_orders_what_validity_does_not() {
        // Two markers, equally valid, neither deprecated. The one the document
        // already uses comes first.
        let source = "\\q1 a\n\\q1 b\n\\q1 c\n";
        let offers = completions(&at_line_start(), source);

        assert!(
            rank(&offers, "q1") < rank(&offers, "q2"),
            "the used marker should outrank the unused one"
        );
    }

    #[test]
    fn ties_break_alphabetically() {
        // Nothing used, nothing deprecated: the order has to be stable and
        // predictable rather than whatever the table happened to hold.
        let offers = completions(&at_line_start(), "");
        let names: Vec<&str> = offers
            .iter()
            .filter(|o| o.valid_here && o.deprecated_in.is_none() && o.uses == 0)
            .map(|o| o.marker)
            .collect();

        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }

    #[test]
    fn an_explicit_marker_brings_its_closing_with_it() {
        // An unclosed \bd is the commonest defect in hand-edited files.
        let offers = completions(&mid_line(), "");
        let bd = &offers[rank(&offers, "bd")];

        assert_eq!(bd.insert, "bd \\bd*");
        // The caret lands where the text goes, not at the end.
        assert_eq!(bd.caret, 3);
    }

    #[test]
    fn nesting_inside_a_character_marker_adds_the_required_prefix() {
        // USFM requires `+` for a character marker inside another one, and
        // getting it wrong is a diagnostic nobody should have to pre-empt.
        let context = Context {
            line_initial: false,
            inside: Some("bd".to_string()),
        };
        let offers = completions(&context, "");
        let it = &offers[rank(&offers, "it")];

        assert_eq!(it.insert, "+it \\+it*");
        assert!(it.valid_here);
    }

    #[test]
    fn a_paragraph_marker_is_not_valid_inside_a_character_marker() {
        let context = Context {
            line_initial: true,
            inside: Some("bd".to_string()),
        };
        let offers = completions(&context, "");
        assert!(!offers[rank(&offers, "p")].valid_here);
    }

    #[test]
    fn a_milestone_closes_itself() {
        let offers = completions(&mid_line(), "");
        let qt = &offers[rank(&offers, "qt-s")];
        assert_eq!(qt.insert, "qt-s\\*");
        assert_eq!(qt.caret, 4);
    }

    #[test]
    fn structural_pseudo_entries_are_never_offered() {
        let offers = completions(&at_line_start(), "");
        assert!(offers.iter().all(|o| o.class != MarkerClass::Unclassified));
    }

    #[test]
    fn frequencies_count_a_marker_and_its_nested_form_together() {
        let counts = frequencies("\\bd a\\bd* \\+bd b\\+bd* \\p\n");

        // Four: the two openings and the two closings, which are the same
        // marker being used.
        assert_eq!(counts.get("bd"), Some(&4));
        assert_eq!(counts.get("p"), Some(&1));
        assert_eq!(counts.get("it"), None);
    }

    #[test]
    fn frequencies_survive_a_trailing_backslash() {
        // A file mid-edit ends in whatever the user just typed.
        assert!(frequencies("\\p text\\").len() <= 2);
        assert_eq!(frequencies("\\").len(), 0);
    }
}
