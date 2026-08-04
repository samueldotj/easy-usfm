//! What the facade produces for real USFM.
//!
//! The corpus suite (P0.1) and the differential oracle (P0.10) are what
//! establish agreement with the ecosystem at scale. These are the small,
//! readable cases that say what the facade is supposed to do at all.

use easy_usfm_core::{Document, NodeKind, Severity};

const GENESIS: &str = "\\id GEN Genesis
\\c 1
\\p
\\v 1 In the beginning God created the heaven and the earth.
\\v 2 And the earth was without form, and void.
";

#[test]
fn the_source_is_returned_exactly_as_given() {
    // ADR-003. Anything else here and byte fidelity is already lost before
    // the editor has been written.
    let document = Document::parse(GENESIS);
    assert_eq!(document.source(), GENESIS);
}

#[test]
fn structure_is_recognized() {
    let document = Document::parse(GENESIS);
    let kinds: Vec<_> = document.descendants().map(|node| node.kind).collect();

    assert!(kinds.contains(&NodeKind::Book));
    assert!(kinds.contains(&NodeKind::Chapter));
    assert!(kinds.contains(&NodeKind::Para));
    assert!(kinds.contains(&NodeKind::Verse));
    assert!(kinds.contains(&NodeKind::Text));
}

#[test]
fn chapters_and_verses_carry_their_numbers() {
    let document = Document::parse(GENESIS);

    let verses: Vec<_> = document
        .descendants()
        .filter(|node| node.kind == NodeKind::Verse)
        .filter_map(|node| node.attribute("number"))
        .collect();

    assert_eq!(verses, vec!["1", "2"]);

    let chapter = document
        .descendants()
        .find(|node| node.kind == NodeKind::Chapter)
        .expect("a chapter");

    assert_eq!(chapter.attribute("number"), Some("1"));
    assert_eq!(chapter.marker.as_ref().map(|m| m.as_str()), Some("c"));
}

#[test]
fn spans_slice_back_to_the_source_they_describe() {
    // The property that matters. A span that does not slice back to its own
    // text puts the cursor in the wrong place, and every feature connecting
    // preview to source inherits the error.
    let document = Document::parse(GENESIS);

    let chapter = document
        .descendants()
        .find(|node| node.kind == NodeKind::Chapter)
        .expect("a chapter");

    let span = chapter.span.as_ref().expect("chapter has a span");
    let text = span.slice(document.source()).expect("span is sliceable");

    assert!(text.starts_with("\\c"), "chapter span covers {text:?}");
}

#[test]
fn every_span_is_in_bounds_and_on_a_character_boundary() {
    // Asserted over text where the three coordinate spaces disagree, because
    // on ASCII they agree and the bug hides. Tamil conjuncts, a Devanagari
    // reordered vowel sign, Hebrew, Arabic, a combining mark, and an astral
    // character.
    let source = "\\id GEN
\\c 1
\\p
\\v 1 க்ஷேமம் क्षि שלום مرحبا e\u{0301} \u{1D400}
\\v 2 \\nd LORD\\nd* \\f + \\ft note\\f*
";

    let document = Document::parse(source);
    let mut spans_seen = 0;

    for node in document.descendants() {
        let Some(span) = node.span.as_ref() else {
            continue;
        };
        spans_seen += 1;

        assert!(
            span.end <= source.len(),
            "{:?} span {span:?} runs past the end of {} bytes",
            node.kind,
            source.len()
        );
        assert!(
            span.slice(source).is_some(),
            "{:?} span {span:?} does not fall on character boundaries",
            node.kind
        );
    }

    assert!(spans_seen > 0, "no spans were checked");
}

#[test]
fn text_survives_in_a_script_that_is_not_latin() {
    let source = "\\id GEN\n\\c 1\n\\p\n\\v 1 க்ஷேமம்\n";
    let document = Document::parse(source);

    let text: String = document
        .descendants()
        .filter(|node| node.kind == NodeKind::Text)
        .filter_map(|node| node.text.as_deref())
        .collect();

    assert!(text.contains("க்ஷேமம்"), "got {text:?}");
}

#[test]
fn character_markers_and_notes_nest() {
    let source = "\\id GEN\n\\c 1\n\\p\n\\v 1 \\nd LORD\\nd* said \\f + \\ft a note\\f*\n";
    let document = Document::parse(source);

    let note = document
        .descendants()
        .find(|node| node.kind == NodeKind::Note)
        .expect("a note");

    assert_eq!(note.marker.as_ref().map(|m| m.as_str()), Some("f"));
    assert!(!note.children.is_empty(), "the note has content");

    assert!(document
        .descendants()
        .any(|node| node.kind == NodeKind::Char
            && node.marker.as_ref().is_some_and(|m| m.as_str() == "nd")));
}

#[test]
fn custom_z_markers_are_carried_rather_than_dropped() {
    // unfoldingWord's alignment data is \zaln-s throughout. ADR-003: content
    // we do not understand still has to survive.
    let source = "\\id GEN\n\\c 1\n\\p\n\\v 1 \\zaln-s |x-lemma=\"אֱלֹהִים\"\\*God\\zaln-e\\*\n";
    let document = Document::parse(source);

    assert!(
        document.descendants().any(|node| node
            .marker
            .as_ref()
            .is_some_and(|marker| marker.as_str().starts_with("zaln"))),
        "no zaln marker survived"
    );
}

#[test]
fn malformed_input_produces_diagnostics_rather_than_nothing() {
    // \bd is never closed.
    let document = Document::parse("\\id GEN\n\\c 1\n\\p\n\\v 1 \\bd unclosed\n");

    let diagnostics = document.diagnostics();
    assert!(
        !diagnostics.is_empty(),
        "an unclosed marker went unreported"
    );

    for diagnostic in diagnostics {
        assert!(
            diagnostic.code.as_str().starts_with("USFM-"),
            "{} has no stable code",
            diagnostic.message
        );
        assert!(
            diagnostic.span.slice(document.source()).is_some(),
            "{} points at {:?}, which is not sliceable",
            diagnostic.code,
            diagnostic.span
        );
    }
}

#[test]
fn diagnostics_are_reported_in_source_order() {
    let document = Document::parse("\\c 1\n\\p\n\\v 1 \\bd one\n\\v 2 \\it two\n");
    let starts: Vec<_> = document
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.span.start)
        .collect();

    assert!(
        starts.windows(2).all(|pair| pair[0] <= pair[1]),
        "out of order: {starts:?}"
    );
}

#[test]
fn a_missing_book_identification_is_an_error() {
    let document = Document::parse("\\c 1\n\\p\n\\v 1 text\n");

    assert!(
        document
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "a document with no \\id reported no error: {:?}",
        document.diagnostics()
    );
}

#[test]
fn degenerate_input_does_not_panic() {
    // Not a substitute for the fuzz target (P0.11) — this is the handful of
    // shapes worth failing fast on, so a regression shows up in a unit test
    // run rather than six hours into a fuzzing session.
    let cases = [
        "",
        "\n",
        "\\",
        "\\\\",
        "\\id",
        "plain text with no marker at all",
        "\\c\n\\v\n",
        "\\v 1 \\f + \\f + \\f + \\f + deeply nested\n",
        "\\id GEN\r\n\\c 1\r\n",
        "\u{feff}\\id GEN\n",
        "\\zi\u{200d}d GEN\n",
        "\\c 999999999999999999999\n",
    ];

    for case in cases {
        let document = Document::parse(case);
        let _ = document.content();
        let _ = document.diagnostics();
    }
}

#[test]
fn parsing_is_deferred_until_something_is_asked_of_it() {
    // ARCHITECTURE §8.1 — the cheap path must not pay for the expensive one.
    // Not a timing assertion, just that the source is available without the
    // tree having been built.
    let document = Document::parse(GENESIS);
    assert_eq!(document.source().len(), GENESIS.len());
    assert!(format!("{document:?}").contains("parsed: false"));

    let _ = document.content();
    assert!(format!("{document:?}").contains("parsed: true"));
}
