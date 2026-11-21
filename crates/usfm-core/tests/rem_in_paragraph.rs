//! A `\rem` inside an open paragraph does not swallow what follows it.
//!
//! The parser nests a `\rem` that arrives while a paragraph is open inside
//! that paragraph. The next block marker closes only the `\rem`, so a `\c` or
//! a `\p` after it becomes a child of the paragraph, where the validator never
//! looks. See `src/backend/remarks.rs`.
//!
//! The case that found it: every book of a published Gujarati New Testament
//! ends its licence introduction with `\ili1 …`, a `\rem` naming the tool
//! that produced it, and then `\c 1`. Every `\p` of chapter 1 was reported as
//! a paragraph before the first chapter, chapter 2 as out of sequence, and
//! the one-chapter books as having no chapter.

use usfm_core::{Diagnostic, DiagnosticCode, Document, Node, NodeKind};

fn usfm(text: &str) -> String {
    text.replace('~', "\\")
}

fn codes(doc: &Document) -> Vec<DiagnosticCode> {
    doc.diagnostics()
        .iter()
        .map(|d: &Diagnostic| d.code)
        .collect()
}

/// Top-level chapter numbers, in order.
fn top_level_chapters(doc: &Document) -> Vec<String> {
    doc.content()
        .iter()
        .filter(|node| node.kind == NodeKind::Chapter)
        .filter_map(|node| node.attribute("number").map(str::to_string))
        .collect()
}

/// Every node in document order, as `kind marker number` strings.
fn walk(doc: &Document) -> Vec<String> {
    fn label(node: &Node) -> String {
        format!(
            "{:?} {} {}",
            node.kind,
            node.marker.as_ref().map(|m| m.as_str()).unwrap_or(""),
            node.attribute("number").unwrap_or(""),
        )
        .trim_end()
        .to_string()
    }
    doc.descendants().map(label).collect()
}

fn text(doc: &Document) -> String {
    doc.descendants()
        .filter(|node| node.kind == NodeKind::Text)
        .filter_map(|node| node.text.as_deref())
        .collect()
}

#[test]
fn a_chapter_after_a_rem_closing_an_introduction_is_a_chapter() {
    let doc = Document::parse(usfm(
        "~id 1JN\n~ili1 licence\n~rem made by a tool\n~c 1\n~cl Chapter 1\n~p\n~v 1 a\n~p\n~v 2 b\n~c 2\n~p\n~v 1 c\n",
    ));

    assert_eq!(top_level_chapters(&doc), ["1", "2"]);
    let got = codes(&doc);
    assert!(
        !got.contains(&DiagnosticCode::BodyParagraphBeforeChapter),
        "{got:?}"
    );
    assert!(
        !got.contains(&DiagnosticCode::InvalidChapterSequence),
        "{got:?}"
    );
    assert!(
        !got.contains(&DiagnosticCode::MissingChapterMarker),
        "{got:?}"
    );
}

#[test]
fn a_one_chapter_book_is_not_reported_as_having_no_chapter() {
    let doc = Document::parse(usfm("~id JUD\n~ip intro\n~rem note\n~c 1\n~p\n~v 1 a\n"));

    assert_eq!(top_level_chapters(&doc), ["1"]);
    assert!(!codes(&doc).contains(&DiagnosticCode::MissingChapterMarker));
}

/// The same fault mid-book: verse numbers restart at a chapter the validator
/// never saw, which it reported as a verse out of sequence.
#[test]
fn a_rem_before_a_chapter_mid_book_does_not_break_verse_numbering() {
    let doc = Document::parse(usfm(
        "~id 1JN\n~c 1\n~p\n~v 1 a\n~rem note\n~c 2\n~p\n~v 1 b\n~v 2 c\n",
    ));

    assert_eq!(top_level_chapters(&doc), ["1", "2"]);
    assert!(
        !codes(&doc).contains(&DiagnosticCode::InvalidVerseSequence),
        "{:?}",
        doc.diagnostics()
    );
}

/// The `\rem` stays where it was written: inside the paragraph, before the
/// chapter.
#[test]
fn the_rem_and_the_text_keep_their_places() {
    let doc = Document::parse(usfm(
        "~id 1JN\n~ili1 licence\n~rem note\n~c 1\n~p\n~v 1 a\n",
    ));

    let order = walk(&doc);
    let at = |label: &str| {
        order
            .iter()
            .position(|x| x == label)
            .unwrap_or_else(|| panic!("{label} in {order:?}"))
    };
    assert!(at("Para ili1") < at("Para rem"));
    assert!(at("Para rem") < at("Chapter c 1"));
    assert!(at("Chapter c 1") < at("Verse v 1"));
    let got = text(&doc);
    assert!(
        got.contains("licence") && got.contains("note") && got.contains('a'),
        "{got:?}"
    );
}

/// What followed the chapter inside the paragraph comes after the chapter,
/// not before it.
#[test]
fn content_after_the_stranded_chapter_stays_after_it() {
    let doc = Document::parse(usfm("~id 1JN\n~ili1 licence\n~rem note\n~c 1\n~v 1 a\n"));

    let order = walk(&doc);
    let chapter = order
        .iter()
        .position(|x| x == "Chapter c 1")
        .expect("chapter");
    let verse = order.iter().position(|x| x == "Verse v 1").expect("verse");
    assert!(chapter < verse, "{order:?}");
    assert_eq!(top_level_chapters(&doc), ["1"]);
    assert!(text(&doc).contains('a'));
}

/// The repair moves blocks; it does not excuse real faults. This one the
/// parser used to hide: the `\p` before the chapter was nested out of the
/// validator's sight, and the error it did report was on the `\p` *after*
/// the chapter, which is fine.
#[test]
fn a_paragraph_really_before_the_first_chapter_is_reported_where_it_is() {
    let source = usfm("~id 1JN\n~ili1 licence\n~rem note\n~p\n~v 1 a\n~c 1\n~p\n~v 1 b\n");
    let early = source.find("\\p\n").expect("first p");
    let doc = Document::parse(source);

    let found: Vec<_> = doc
        .diagnostics()
        .iter()
        .filter(|d| d.code == DiagnosticCode::BodyParagraphBeforeChapter)
        .map(|d| d.span.start)
        .collect();
    assert_eq!(found, [early], "{:?}", doc.diagnostics());
    assert_eq!(top_level_chapters(&doc), ["1"]);
}

/// A `\p` after a `\rem` in the body is the next paragraph, not part of the
/// one the remark was in.
#[test]
fn a_paragraph_after_a_rem_is_a_sibling_not_a_child() {
    let doc = Document::parse(usfm(
        "~id 1JN\n~c 1\n~p\n~v 1 a\n~rem note\n~p\n~v 2 b\n~c 2\n~p\n~v 1 c\n",
    ));

    let top: Vec<_> = doc
        .content()
        .iter()
        .map(|n| {
            format!(
                "{:?} {}",
                n.kind,
                n.marker.as_ref().map(|m| m.as_str()).unwrap_or("")
            )
        })
        .collect();
    assert_eq!(
        top,
        [
            "Book id",
            "Chapter c",
            "Para p",
            "Para p",
            "Chapter c",
            "Para p"
        ]
    );
    assert!(codes(&doc).is_empty(), "{:?}", doc.diagnostics());
}

#[test]
fn a_real_gap_in_chapters_is_still_reported() {
    let doc = Document::parse(usfm(
        "~id 1JN\n~c 1\n~p\n~v 1 a\n~rem note\n~c 3\n~p\n~v 1 b\n",
    ));

    let found: Vec<_> = doc
        .diagnostics()
        .iter()
        .filter(|d| d.code == DiagnosticCode::InvalidChapterSequence)
        .map(|d| d.message.clone())
        .collect();
    assert_eq!(found, ["expected chapter 2 but found chapter 3"]);
}

/// A `\rem` inside a paragraph with no chapter after it is left exactly as
/// the parser built it.
#[test]
fn a_rem_with_no_chapter_after_it_is_untouched() {
    let doc = Document::parse(usfm("~id 1JN\n~c 1\n~p\n~v 1 a\n~rem note\n~v 2 b\n"));

    assert_eq!(top_level_chapters(&doc), ["1"]);
    assert!(text(&doc).contains("note"));
    assert!(codes(&doc).is_empty(), "{:?}", doc.diagnostics());
}
