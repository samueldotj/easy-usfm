//! A `|` outside a character marker is punctuation, not an attribute block.
//!
//! The parser reads every pipe as the start of an attribute block and, for a
//! paragraph — which has nowhere to put attributes — throws the result away.
//! That deletes text with no diagnostic. See `src/backend/pipes.rs`.
//!
//! This matters for Indic scripts in particular: `|` is the **danda**, the
//! full stop of Sanskrit-derived writing, so a published translation can use
//! it in every sentence.

use usfm_core::{Document, NodeKind};

/// All the text in the document, in order.
fn text(doc: &Document) -> String {
    let mut out = String::new();
    for node in doc.descendants() {
        if node.kind == NodeKind::Text {
            if let Some(t) = &node.text {
                out.push_str(t);
            }
        }
    }
    out
}

#[test]
fn a_pipe_mid_line_does_not_swallow_the_rest_of_it() {
    let doc = Document::parse("\\id 3JN\n\\c 1\n\\p\n\\v 11 before| after more words\n");
    let got = text(&doc);

    assert!(got.contains("before"), "{got:?}");
    assert!(
        got.contains("after more words"),
        "text after the pipe was lost: {got:?}"
    );
    // The pipe is the punctuation, so it comes back too.
    assert!(got.contains('|'), "the danda itself was lost: {got:?}");
}

/// The subtler half of the same bug. `parse_attributes("|")` yields an empty
/// list, which the builder also discards — so a danda at the end of a line
/// disappeared even when no words followed it.
#[test]
fn a_pipe_at_the_end_of_a_line_survives() {
    let doc = Document::parse("\\id 3JN\n\\c 1\n\\p\n\\v 6 a sentence|\n\\v 7 another|\n");
    let got = text(&doc);

    assert_eq!(got.matches('|').count(), 2, "{got:?}");
}

/// The regression this repair must not cause: a real attribute block on a
/// character marker is still an attribute block.
#[test]
fn a_real_attribute_is_still_consumed() {
    let doc = Document::parse(
        "\\id 3JN\n\\c 1\n\\p\n\\v 1 word \\w grace|lemma=\"grace\" strong=\"G5485\"\\w* after\n",
    );
    let got = text(&doc);

    assert!(
        !got.contains('|'),
        "attribute syntax leaked into text: {got:?}"
    );
    assert!(!got.contains("lemma"), "{got:?}");

    let w = doc
        .descendants()
        .find(|n| n.marker.as_ref().is_some_and(|m| m.as_str() == "w"))
        .expect("the \\w node");
    assert_eq!(w.attribute("lemma"), Some("grace"));
    assert_eq!(w.attribute("strong"), Some("G5485"));
}

#[test]
fn a_figure_keeps_its_attributes_and_its_caption() {
    let doc = Document::parse(
        "\\id MRK\n\\c 1\n\\p\n\\v 1 \\fig The boat|src=\"boat.png\" size=\"col\"\\fig*\n",
    );
    let got = text(&doc);

    assert!(got.contains("The boat"), "{got:?}");
    assert!(!got.contains("boat.png"), "attributes leaked: {got:?}");

    let fig = doc
        .descendants()
        .find(|n| n.kind == NodeKind::Figure)
        .expect("the figure");
    assert_eq!(fig.attribute("file"), Some("boat.png"));
}

/// Restored text has to land where it was written, not at the end of the
/// paragraph — otherwise a sentence reads in the wrong order.
#[test]
fn restored_text_keeps_its_position() {
    let doc = Document::parse("\\id 3JN\n\\c 1\n\\p\n\\v 1 one| two \\add three\\add* four\n");
    let got = text(&doc);

    let (one, two) = (got.find("one").expect("one"), got.find("two").expect("two"));
    let (three, four) = (
        got.find("three").expect("three"),
        got.find("four").expect("four"),
    );
    assert!(
        one < two && two < three && three < four,
        "out of order: {got:?}"
    );
}

#[test]
fn several_pipes_on_one_line_all_come_back() {
    let doc = Document::parse("\\id 3JN\n\\c 1\n\\p\n\\v 1 a| b \\add c\\add* d| e\n");
    let got = text(&doc);

    assert_eq!(got.matches('|').count(), 2, "{got:?}");
    for word in ["a", "b", "c", "d", "e"] {
        assert!(got.contains(word), "{word} missing from {got:?}");
    }
}

#[test]
fn a_pipe_inside_a_table_cell_survives() {
    let doc = Document::parse("\\id 3JN\n\\c 1\n\\tr \\tc1 first| second \\tc2 third\n");
    let got = text(&doc);

    assert!(got.contains("second"), "{got:?}");
    assert!(got.contains("third"), "{got:?}");
}

/// The shape that started this: a real verse from a Sanskrit translation,
/// where the danda ends every clause.
#[test]
fn an_indic_verse_keeps_every_clause() {
    let doc = Document::parse(concat!(
        "\\id 3JN\n\\c 1\n\\p\n",
        "\\v 11 হে প্ৰিয, ৎৱযা দুষ্কৰ্ম্ম নানুক্ৰিযতাং কিন\u{9cd}তু সৎকৰ্ম্মৈৱ| ",
        "যঃ সৎকৰ্ম্মাচাৰী স ঈশ্ৱৰাৎ জাতঃ, যো দুষ্কৰ্ম্মাচাৰী স ঈশ্ৱৰং ন দৃষ্টৱান্|\n"
    ));
    let got = text(&doc);

    assert!(
        got.contains("সৎকৰ্ম্মাচাৰী"),
        "the clause after the danda was lost: {got:?}"
    );
    assert_eq!(got.matches('|').count(), 2, "{got:?}");
}
