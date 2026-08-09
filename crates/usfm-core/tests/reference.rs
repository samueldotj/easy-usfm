//! Resolving references against a document — P2.9.
//!
//! The acceptance criterion is that *all accepted reference forms* resolve,
//! "including `\vp` fallback and non-ASCII digits". Those two are the ones
//! that fail silently: a `\vp` file is one where the number on the page is not
//! the number in the markup, so a reader typing what they see finds nothing
//! and has no way to know why.

use usfm_core::{Resolution, Session};

/// Tamil Genesis, where every verse also carries its published number.
fn tamil() -> Session {
    Session::new(
        "\\id GEN\n\
         \\c 1\n\
         \\p\n\
         \\v 1 \\vp \u{BE7}\\vp* \u{B86}\u{BB0}\u{BAE}\u{BCD}\u{BAA}\u{BAE}\n\
         \\v 2 \\vp \u{BE8}\\vp* \u{B87}\u{BB0}\u{BA3}\u{BCD}\u{B9F}\u{BC1}\n\
         \\v 3 \\vp \u{BE9}\\vp* \u{BAE}\u{BC2}\u{BA9}\u{BCD}\u{BB1}\u{BC1}\n\
         \\c 2\n\
         \\p\n\
         \\v 1 second chapter\n",
    )
}

fn plain() -> Session {
    Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\v 2 b\n\\c 2\n\\p\n\\v 1 c\n\\v 2-3 d\n")
}

/// Where a reference landed, as a byte offset.
///
/// The span a verse resolves to is the `\v` marker token itself, so what is
/// checked is *where* it points rather than what it slices.
fn offset(session: &Session, text: &str) -> usize {
    match session.resolve(text) {
        Resolution::Found(span) => span.start,
        other => panic!("{text:?} did not resolve: {other:?}"),
    }
}

/// The offset of the nth occurrence of `needle`, counting from one.
fn nth(source: &str, needle: &str, n: usize) -> usize {
    source
        .match_indices(needle)
        .nth(n - 1)
        .unwrap_or_else(|| panic!("{needle:?} does not occur {n} times"))
        .0
}

#[test]
fn every_accepted_form_resolves() {
    let session = plain();
    let source = session.source();

    // PRODUCT §6.2 names these four exactly.
    let first_verse = nth(source, "\\v 1 a", 1);
    assert_eq!(offset(&session, "GEN 1:1"), first_verse);
    assert_eq!(offset(&session, "Gen 1.1"), first_verse);
    assert_eq!(offset(&session, "1:1"), first_verse);
    assert_eq!(offset(&session, "2"), nth(source, "\\c 2", 1));
}

#[test]
fn a_chapter_resolves_to_its_own_marker_line() {
    // Not to the first verse under it. A chapter with nothing in it yet still
    // has somewhere to put the cursor, which is the state a file is in while
    // someone is typing it.
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\c 2\n");
    match session.resolve("2") {
        Resolution::Found(span) => assert_eq!(span.slice(session.source()), Some("\\c 2\n")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_range_is_reachable_by_either_number() {
    let session = plain();
    let range = nth(session.source(), "\\v 2-3 d", 1);

    assert_eq!(offset(&session, "2:2"), range);
    assert_eq!(offset(&session, "2:3"), range);
}

#[test]
fn a_published_number_is_found_by_what_the_reader_sees() {
    let session = tamil();
    // Verse 3, typed in Tamil digits as the page prints it.
    assert_eq!(
        offset(&session, "1:\u{BE9}"),
        nth(session.source(), "\\v 3", 1)
    );
}

#[test]
fn a_published_number_is_found_when_it_disagrees_with_the_v_number() {
    // The case \vp exists for: the markup counts one way and the page counts
    // another. Someone reading the page types 7 and must land on \v 5.
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 4 \\vp 6\\vp* d\n\\v 5 \\vp 7\\vp* e\n");
    let verse_five = nth(session.source(), "\\v 5", 1);

    assert_eq!(offset(&session, "1:7"), verse_five);
    // And the markup number still works, because both are real.
    assert_eq!(offset(&session, "1:5"), verse_five);
}

#[test]
fn the_v_number_wins_when_both_could_match() {
    // \v 4 has \vp 6, and there is also a real \v 6. Typing 6 must reach the
    // verse actually numbered 6 -- the markup is the more specific answer, and
    // the fallback is a fallback.
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 4 \\vp 6\\vp* d\n\\v 6 \\vp 8\\vp* f\n");
    assert_eq!(offset(&session, "1:6"), nth(session.source(), "\\v 6", 1));
}

#[test]
fn non_ascii_digits_resolve_identically_to_ascii() {
    // UNICODE §6: "௩:௧ and 3:1 resolve identically".
    let session = plain();

    assert_eq!(
        session.resolve("\u{BE8}:\u{BE7}"),
        session.resolve("2:1"),
        "Tamil and ASCII must reach the same verse"
    );
    assert_eq!(
        offset(&session, "\u{BE8}:\u{BE7}"),
        nth(session.source(), "\\v 1 c", 1)
    );

    // A script that appears nowhere in the implementation.
    assert_eq!(session.resolve("\u{966}\u{968}"), session.resolve("2"));
}

#[test]
fn a_tamil_published_number_is_reachable_from_an_ascii_keyboard() {
    // The translator has a Tamil keyboard; the consultant reviewing the file
    // does not. Both have to be able to type the reference on the page.
    let session = tamil();
    assert_eq!(session.resolve("1:2"), session.resolve("1:\u{BE8}"));
}

#[test]
fn a_reference_to_another_book_says_so_rather_than_failing() {
    // A one-file editor: this is not a typo to correct but a document to open,
    // and "not found" would send the user looking for the wrong problem.
    let session = plain();
    match session.resolve("EXO 1:1") {
        Resolution::WrongBook { document, asked } => {
            assert_eq!(document.as_deref(), Some("GEN"));
            assert_eq!(asked, "EXO");
        }
        other => panic!("expected a wrong-book answer, got {other:?}"),
    }
}

#[test]
fn a_document_that_does_not_say_what_it_is_accepts_any_book() {
    // A file with no \id is one this editor should still navigate.
    let session = Session::new("\\c 1\n\\p\n\\v 1 a\n");
    assert!(matches!(session.resolve("GEN 1:1"), Resolution::Found(_)));
}

#[test]
fn what_is_missing_is_named() {
    let session = plain();

    assert!(matches!(session.resolve("9"), Resolution::NoSuchChapter(9)));
    assert!(matches!(
        session.resolve("9:1"),
        Resolution::NoSuchChapter(9)
    ));

    match session.resolve("1:99") {
        Resolution::NoSuchVerse { chapter, verse } => {
            assert_eq!((chapter, verse.as_str()), (1, "99"));
        }
        other => panic!("expected a missing verse, got {other:?}"),
    }

    // A word on its own is a book code, so this is a different book rather
    // than a missing chapter — which is the more useful thing to be told.
    assert!(matches!(
        session.resolve("what"),
        Resolution::WrongBook { .. }
    ));
    assert!(matches!(session.resolve(""), Resolution::Unparseable));
}

#[test]
fn the_cursor_position_reads_back_as_a_reference() {
    let session = plain();
    let source = session.source();

    assert_eq!(
        session.reference_at(nth(source, "\\v 1 a", 1)).as_deref(),
        Some("GEN 1:1")
    );
    // Inside the verse's text, not on its marker.
    assert_eq!(
        session
            .reference_at(nth(source, "\\v 2 b", 1) + 5)
            .as_deref(),
        Some("GEN 1:2")
    );
    assert_eq!(
        session.reference_at(nth(source, "\\v 2-3 d", 1)).as_deref(),
        Some("GEN 2:2-3")
    );
    // Before any verse there is nothing to report.
    assert_eq!(session.reference_at(0), None);
}

#[test]
fn a_position_between_chapters_reports_the_chapter_it_is_in() {
    // A cursor on the `\c 2` line sits after the last verse of chapter 1, so
    // the verse index alone answers 1:2 — the status bar naming the wrong
    // chapter at the exact moment the user has navigated to a new one.
    let session = plain();
    let at = nth(session.source(), "\\c 2", 1);

    assert_eq!(session.reference_at(at).as_deref(), Some("GEN 2"));
}

#[test]
fn a_chapter_with_no_verses_yet_still_reports_where_it_is() {
    // The state a file is in while it is being typed.
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\c 2\n\\p\n");
    let at = nth(session.source(), "\\c 2", 1);

    assert_eq!(session.reference_at(at).as_deref(), Some("GEN 2"));
}

#[test]
fn the_status_bar_shows_the_number_on_the_page() {
    // Where \vp disagrees with \v, the reader's number is the useful one.
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 4 \\vp 6\\vp* d\n");
    let at = nth(session.source(), "\\v 4", 1);
    assert_eq!(session.reference_at(at).as_deref(), Some("GEN 1:6"));
}
