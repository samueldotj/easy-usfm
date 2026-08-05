//! The incremental session — P0.4.
//!
//! Two things are being established. That an ordinary edit costs one chunk,
//! which is the performance claim. And that `\c` insertion and deletion split
//! and merge correctly, which is the correctness claim and the one that can
//! corrupt a document silently: a mis-chunked document parses each half as
//! nonsense while looking entirely normal on screen.
//!
//! Agreement between chunked and whole-document parsing is asserted across the
//! corpus by P0.5. What is here is the mechanics.

use easy_usfm_core::{ByteSpan, Document, Edit, EditError, NodeKind, Session, Version};

fn book(chapters: usize, verses_per_chapter: usize) -> String {
    let mut text = String::from("\\id GEN Genesis\n\\h Genesis\n");
    for chapter in 1..=chapters {
        text.push_str(&format!("\\c {chapter}\n\\p\n"));
        for verse in 1..=verses_per_chapter {
            text.push_str(&format!("\\v {verse} In the beginning God created.\n"));
        }
    }
    text
}

// ------------------------------------------------------------- chunking ---

#[test]
fn a_document_partitions_at_chapter_markers() {
    let session = Session::new(book(3, 2));
    let numbers: Vec<_> = session.chunks().iter().map(|c| c.number()).collect();

    // The header chunk carries everything before \c 1.
    assert_eq!(numbers, vec![None, Some(1), Some(2), Some(3)]);
}

#[test]
fn chunks_tile_the_document_exactly() {
    // Any gap or overlap loses or duplicates text, and both are invisible
    // until something is edited near the seam.
    let session = Session::new(book(4, 3));

    let mut expected_start = 0;
    for chunk in session.chunks() {
        assert_eq!(chunk.range().start, expected_start, "gap or overlap");
        expected_start = chunk.range().end;
    }
    assert_eq!(expected_start, session.source().len());
}

#[test]
fn a_document_with_no_chapter_is_a_single_header_chunk() {
    let session = Session::new("\\id GEN\n\\h Genesis\n");
    assert_eq!(session.chunks().len(), 1);
    assert_eq!(session.chunks()[0].number(), None);
}

#[test]
fn a_document_starting_at_a_chapter_has_no_header_chunk() {
    let session = Session::new("\\c 1\n\\p\n\\v 1 text\n");
    assert_eq!(session.chunks().len(), 1);
    assert_eq!(session.chunks()[0].number(), Some(1));
}

#[test]
fn markers_that_merely_begin_with_c_are_not_boundaries() {
    // \cl, \cp, and \ca all start with the same two characters and none is a
    // chapter boundary. Treating one as a boundary splits a chapter at its own
    // published number, and both halves then parse as nonsense.
    let session = Session::new("\\c 1\n\\cl Psalm\n\\cp A\n\\ca 2\\ca*\n\\p\n\\v 1 text\n");
    assert_eq!(session.chunks().len(), 1);
    assert_eq!(session.chunks()[0].number(), Some(1));
}

#[test]
fn a_chapter_marker_that_is_not_line_initial_is_not_a_boundary() {
    // \c is a synchronization point at line start. Mid-line it is malformed
    // content, and splitting there would be wrong.
    let session = Session::new("\\c 1\n\\p\n\\v 1 text \\c 2 still verse one\n");
    assert_eq!(session.chunks().len(), 1);
}

#[test]
fn an_unnumbered_chapter_still_opens_a_chunk() {
    // The number is missing, which is a diagnostic. The synchronization point
    // is still there, which is a fact about the text.
    let session = Session::new("\\id GEN\n\\c\n\\p\n\\v 1 text\n");
    assert_eq!(session.chunks().len(), 2);
    assert_eq!(session.chunks()[1].number(), None);
}

// ---------------------------------------------------------- incremental ---

#[test]
fn an_edit_inside_a_chapter_invalidates_only_that_chapter() {
    // The claim the whole design rests on.
    let mut session = Session::new(book(10, 5));

    let target = session.chunks()[5].range();
    let at = target.start + 20;
    let applied = session
        .edit(ByteSpan::new(at, at), "X")
        .expect("edit applies");

    assert_eq!(
        applied.invalidated, 1,
        "more than one chunk was invalidated"
    );
    assert_eq!(applied.shifted, 5, "later chapters should only have moved");
}

#[test]
fn chunks_after_an_edit_keep_their_parse() {
    // Cached parses survive a shift only because the offsets inside them are
    // chunk-relative. If that ever changes, this is what notices.
    let mut session = Session::new(book(6, 4));
    let _ = session.content(); // force every chunk to parse

    let at = session.chunks()[2].range().start + 10;
    session
        .edit(ByteSpan::new(at, at), "Y")
        .expect("edit applies");

    let parsed_after: Vec<bool> = session.chunks().iter().map(|c| c.is_parsed()).collect();
    assert_eq!(
        parsed_after,
        vec![true, true, false, true, true, true, true],
        "only the edited chunk should have lost its parse"
    );
}

#[test]
fn spans_stay_correct_in_chapters_after_an_edit() {
    // The bug chunk-relative storage exists to prevent: a stale offset in a
    // later chapter, which puts the cursor in the wrong place and looks like
    // a rendering fault rather than an arithmetic one.
    let mut session = Session::new(book(4, 3));
    let _ = session.content();

    let at = session.chunks()[1].range().start + 5;
    session
        .edit(ByteSpan::new(at, at), "INSERTED")
        .expect("edit applies");

    for (index, chunk) in session.chunks().iter().enumerate() {
        for node in session.chunk_content(index) {
            let Some(span) = node.span.as_ref() else {
                continue;
            };
            assert!(
                span.start >= chunk.range().start && span.end <= chunk.range().end,
                "{:?} span {span:?} escaped chunk {:?}",
                node.kind,
                chunk.range()
            );
            assert!(
                span.slice(session.source()).is_some(),
                "span {span:?} is not sliceable after the edit"
            );
        }
    }
}

#[test]
fn every_revision_is_recorded_on_the_chunks_that_changed() {
    let mut session = Session::new(book(5, 3));
    assert_eq!(session.rev(), 0);

    let at = session.chunks()[3].range().start + 8;
    session
        .edit(ByteSpan::new(at, at), "Z")
        .expect("edit applies");

    assert_eq!(session.rev(), 1);
    let revised: Vec<u64> = session.chunks().iter().map(|c| c.rev()).collect();
    assert_eq!(revised, vec![0, 0, 0, 1, 0, 0]);
}

// ------------------------------------------------------- split and merge ---

#[test]
fn inserting_a_chapter_marker_splits_a_chunk() {
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 one\n\\v 2 two\n");
    assert_eq!(session.chunks().len(), 2);

    let at = session.source().find("\\v 2").expect("verse two");
    session
        .edit(ByteSpan::new(at, at), "\\c 2\n\\p\n")
        .expect("edit applies");

    let numbers: Vec<_> = session.chunks().iter().map(|c| c.number()).collect();
    assert_eq!(numbers, vec![None, Some(1), Some(2)]);
}

#[test]
fn deleting_a_chapter_marker_merges_with_the_chunk_before_it() {
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 one\n\\c 2\n\\p\n\\v 1 two\n");
    assert_eq!(session.chunks().len(), 3);

    let at = session.source().find("\\c 2\n").expect("chapter two");
    session
        .edit(ByteSpan::new(at, at + "\\c 2\n".len()), "")
        .expect("edit applies");

    let numbers: Vec<_> = session.chunks().iter().map(|c| c.number()).collect();
    assert_eq!(numbers, vec![None, Some(1)], "chapter 2 should have merged");
    assert!(session.source().contains("\\v 1 two"), "text was lost");
}

#[test]
fn breaking_a_chapter_marker_merges_it_away() {
    // Deleting only the backslash. The line is no longer a marker, so the
    // boundary is gone and the chunk has to merge backwards -- which is only
    // possible if the previous chunk is rebuilt too.
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 one\n\\c 2\n\\p\n\\v 1 two\n");
    let at = session.source().find("\\c 2").expect("chapter two");

    session
        .edit(ByteSpan::new(at, at + 1), "")
        .expect("edit applies");

    let numbers: Vec<_> = session.chunks().iter().map(|c| c.number()).collect();
    assert_eq!(numbers, vec![None, Some(1)]);
}

#[test]
fn repairing_a_chapter_marker_splits_it_back_out() {
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 one\nc 2\n\\p\n\\v 1 two\n");
    assert_eq!(session.chunks().len(), 2);

    let at = session.source().find("c 2").expect("broken marker");
    session
        .edit(ByteSpan::new(at, at), "\\")
        .expect("edit applies");

    let numbers: Vec<_> = session.chunks().iter().map(|c| c.number()).collect();
    assert_eq!(numbers, vec![None, Some(1), Some(2)]);
}

#[test]
fn joining_two_lines_across_a_boundary_merges_the_chunks() {
    // Deleting the newline before \c 2 puts the marker mid-line, where it is
    // no longer a synchronization point.
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 one\n\\c 2\n\\p\n\\v 1 two\n");
    let newline = session.source().find("\\c 2").expect("chapter two") - 1;

    session
        .edit(ByteSpan::new(newline, newline + 1), "")
        .expect("edit applies");

    let numbers: Vec<_> = session.chunks().iter().map(|c| c.number()).collect();
    assert_eq!(numbers, vec![None, Some(1)]);
}

#[test]
fn typing_a_chapter_marker_one_character_at_a_time_ends_in_one_chunk_per_chapter() {
    // The realistic version of the split case, and the one that catches a
    // boundary rule that only works on whole-line edits.
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 one\n");

    let mut at = session.source().len();
    for character in "\\c 2\n".chars() {
        let mut buffer = [0u8; 4];
        let text = character.encode_utf8(&mut buffer);
        session
            .edit(ByteSpan::new(at, at), text)
            .expect("edit applies");
        at += text.len();
    }

    let numbers: Vec<_> = session.chunks().iter().map(|c| c.number()).collect();
    assert_eq!(numbers, vec![None, Some(1), Some(2)]);
}

// ------------------------------------------------------------ agreement ---

/// The chunked tree must say the same thing as parsing the whole document.
/// P0.5 asserts this across the corpus; this is the smoke test.
fn assert_agrees_with_whole_document(source: &str) {
    let session = Session::new(source.to_string());
    let document = Document::parse(source.to_string());

    let chunked: Vec<_> = session
        .content()
        .iter()
        .flat_map(|node| {
            node.descendants()
                .map(|n| (n.kind, n.span.clone()))
                .collect::<Vec<_>>()
        })
        .filter(|(kind, _)| *kind == NodeKind::Chapter || *kind == NodeKind::Verse)
        .collect();

    let whole: Vec<_> = document
        .descendants()
        .map(|n| (n.kind, n.span.clone()))
        .filter(|(kind, _)| *kind == NodeKind::Chapter || *kind == NodeKind::Verse)
        .collect();

    assert_eq!(chunked, whole, "chunked parse disagrees on\n{source}");
}

#[test]
fn chunked_parsing_agrees_with_whole_document_parsing() {
    assert_agrees_with_whole_document(&book(3, 3));
    assert_agrees_with_whole_document(
        "\\id GEN\n\\c 1\n\\p\n\\v 1 க்ஷேமம்\n\\c 2\n\\p\n\\v 1 שלום\n",
    );
    assert_agrees_with_whole_document("\\c 1\n\\p\n\\v 1 no header chunk\n");
}

#[test]
fn a_chapter_chunk_does_not_report_the_missing_book_identification() {
    // Every chapter parsed alone lacks \id, and the parser correctly says so
    // for the text it was given. Reporting it once per chapter would bury the
    // real diagnostics under a hundred copies of a non-problem.
    let session = Session::new(book(5, 2));
    let missing_id = session
        .diagnostics()
        .iter()
        .filter(|d| d.code == easy_usfm_core::DiagnosticCode::MissingIdMarker)
        .count();

    assert_eq!(missing_id, 0, "chapter chunks reported a missing \\id");
}

#[test]
fn a_document_with_chapters_does_not_report_them_missing() {
    // The header chunk is *defined* as everything before the first `\c`, so it
    // never contains one and the parser always reports it missing there.
    // Treating the header as the authority put a false Error on every
    // well-formed document in the corpus, including this one.
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 hello\n");

    assert_eq!(
        session.diagnostics(),
        vec![],
        "a well-formed document reported a diagnostic"
    );
}

#[test]
fn a_document_with_no_chapters_still_reports_them_missing() {
    // Suppressing it per chunk must not lose it. The question moved to Tier 3,
    // where the chunk list is visible; it did not go away.
    let session = Session::new("\\id GEN\n\\p\n\\v 1 no chapter anywhere\n");
    let missing: Vec<_> = session
        .diagnostics()
        .into_iter()
        .filter(|d| d.code == easy_usfm_core::DiagnosticCode::MissingChapterMarker)
        .collect();

    assert_eq!(missing.len(), 1, "expected exactly one, got {missing:?}");
    assert_eq!(missing[0].span, ByteSpan::new(0, 0));
}

#[test]
fn a_chapter_is_not_reported_missing_once_per_chapter() {
    // The failure mode the per-chunk filter exists to prevent, in the other
    // direction: five chapters must not mean five copies of anything.
    let session = Session::new(book(5, 2));
    let missing = session
        .diagnostics()
        .iter()
        .filter(|d| d.code == easy_usfm_core::DiagnosticCode::MissingChapterMarker)
        .count();

    assert_eq!(missing, 0);
}

// --------------------------------------------------------------- version ---

#[test]
fn a_document_that_declares_nothing_is_reported_as_declaring_nothing() {
    // Not the same as declaring the assumed version. Most files in circulation
    // carry no \usfm line and are valid, so the status bar has to be able to
    // say "assumed" rather than claim a declaration the file never made.
    let session = Session::new("\\id GEN\n\\c 1\n");

    assert_eq!(session.detected_version(), None);
    assert_eq!(session.document_version(), Version::ASSUMED);
    assert!(!session.version_is_overridden());
}

#[test]
fn typing_a_usfm_line_changes_the_version_without_reopening() {
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n");
    assert_eq!(session.detected_version(), None);

    session
        .edit(ByteSpan::new(8, 8), "\\usfm 3.1\n")
        .expect("insert the declaration");

    assert_eq!(session.detected_version(), Some(Version::V3_1));
    assert_eq!(session.document_version(), Version::V3_1);
}

#[test]
fn deleting_the_usfm_line_returns_to_the_assumed_version() {
    let source = "\\id GEN\n\\usfm 2.0\n\\c 1\n";
    let mut session = Session::new(source);
    assert_eq!(session.detected_version(), Some(Version::V2_0));

    session
        .edit(ByteSpan::new(8, 18), "")
        .expect("delete the declaration");

    assert_eq!(session.detected_version(), None);
    assert_eq!(session.document_version(), Version::ASSUMED);
}

#[test]
fn an_override_beats_the_declaration_and_survives_an_edit() {
    // Someone who has said "treat this as 2.0" has not asked to be
    // second-guessed the next time they touch the header.
    let mut session = Session::new("\\id GEN\n\\usfm 3.1\n\\c 1\n\\p\n\\v 1 a\n");
    session.override_version(Some(Version::V2_0));

    assert_eq!(session.document_version(), Version::V2_0);
    assert_eq!(session.detected_version(), Some(Version::V3_1));
    assert!(session.version_is_overridden());

    session
        .edit(ByteSpan::new(31, 31), "b")
        .expect("type in the body");
    assert_eq!(session.document_version(), Version::V2_0);

    // Clearing it returns to what the file says, not to the assumed default.
    session.override_version(None);
    assert_eq!(session.document_version(), Version::V3_1);
    assert!(!session.version_is_overridden());
}

#[test]
fn overriding_the_version_shifts_severity_without_reparsing() {
    // \esb arrived in 3.1. Against a document declaring 3.0 that is worth
    // mentioning; against one declaring 3.1 it is not. Nothing about the text
    // changes between these two states -- only the judgement.
    let source = "\\id GEN\n\\usfm 3.0\n\\c 1\n\\p\n\\esb\n\\p body\n\\esbe\n";
    let mut session = Session::new(source);

    let newer = |session: &Session| {
        session
            .diagnostics()
            .iter()
            .filter(|d| d.code == easy_usfm_core::DiagnosticCode::MarkerNewerThanDocument)
            .count()
    };

    assert_eq!(newer(&session), 1, "3.0 should flag \\esb as newer");

    session.override_version(Some(Version::V3_1));
    assert_eq!(newer(&session), 0, "3.1 should not");

    session.override_version(Some(Version::V2_0));
    assert!(newer(&session) >= 1, "2.0 should flag it again");
}

#[test]
fn duplicate_chapters_are_caught_across_chunks() {
    // No chunk can see this on its own -- it is Tier 3's to report.
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 a\n\\c 1\n\\p\n\\v 1 b\n");
    assert!(
        session
            .diagnostics()
            .iter()
            .any(|d| d.code == easy_usfm_core::DiagnosticCode::DuplicateChapter),
        "a repeated chapter number went unreported"
    );
}

#[test]
fn diagnostics_are_in_source_order_across_chunks() {
    let session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 \\bd a\n\\c 2\n\\p\n\\v 1 \\it b\n");
    let starts: Vec<_> = session.diagnostics().iter().map(|d| d.span.start).collect();
    assert!(starts.windows(2).all(|w| w[0] <= w[1]), "{starts:?}");
}

// ----------------------------------------------------------- edit safety ---

#[test]
fn a_batch_applies_in_original_document_coordinates() {
    // The coordinates CodeMirror's iterChanges reports. Each edit must be
    // interpreted against the document as it was before the batch, not after
    // the previous edit in the same batch.
    let mut session = Session::new("\\id GEN\n\\c 1\n\\p\n\\v 1 AAA BBB\n");
    let a = session.source().find("AAA").unwrap();
    let b = session.source().find("BBB").unwrap();

    session
        .apply(&[
            Edit::new(ByteSpan::new(a, a + 3), "one"),
            Edit::new(ByteSpan::new(b, b + 3), "two"),
        ])
        .expect("batch applies");

    assert!(
        session.source().contains("\\v 1 one two"),
        "{}",
        session.source()
    );
}

#[test]
fn an_edit_inside_a_character_is_refused() {
    // Applying it would desynchronise the mirrored buffer, and a
    // desynchronised mirror corrupts every offset in the interface.
    let mut session = Session::new("\\c 1\n\\p\n\\v 1 க\n");
    let inside = session.source().find('க').unwrap() + 1;

    assert!(matches!(
        session.edit(ByteSpan::new(inside, inside), "x"),
        Err(EditError::NotOnCharBoundary { .. })
    ));
}

#[test]
fn an_edit_past_the_end_is_refused() {
    let mut session = Session::new("\\c 1\n");
    let past = session.source().len() + 10;
    assert!(matches!(
        session.edit(ByteSpan::new(past, past), "x"),
        Err(EditError::OutOfBounds { .. })
    ));
}

#[test]
fn overlapping_edits_are_refused() {
    let mut session = Session::new("\\c 1\n\\p\n\\v 1 abcdef\n");
    assert!(matches!(
        session.apply(&[
            Edit::new(ByteSpan::new(10, 14), "x"),
            Edit::new(ByteSpan::new(12, 16), "y"),
        ]),
        Err(EditError::Overlapping { .. })
    ));
}

#[test]
fn editing_the_header_chunk_works() {
    let mut session = Session::new(book(3, 2));
    let applied = session
        .edit(ByteSpan::new(4, 7), "EXO")
        .expect("edit applies");

    assert_eq!(applied.invalidated, 1);
    assert!(session.source().starts_with("\\id EXO"));
}

#[test]
fn deleting_everything_leaves_one_empty_chunk() {
    let mut session = Session::new(book(2, 2));
    let length = session.source().len();
    session
        .edit(ByteSpan::new(0, length), "")
        .expect("edit applies");

    assert_eq!(session.source(), "");
    assert_eq!(session.chunks().len(), 1);
    let _ = session.content();
}

// ------------------------------------------------------------ performance ---

/// ARCHITECTURE §11: a single-chapter edit on a 2 MB document reparses in
/// under 15 ms.
///
/// Release only. A debug build is roughly an order of magnitude slower and
/// would fail on a correct implementation, so asserting the budget there would
/// train everyone to ignore it. CI runs this in release.
#[test]
#[cfg(not(debug_assertions))]
fn a_single_chapter_edit_reparses_a_2mb_document_within_budget() {
    use std::time::Instant;

    let mut source = String::from("\\id GEN Genesis\n\\h Genesis\n");
    let mut chapter = 1;
    while source.len() < 2 * 1024 * 1024 {
        source.push_str(&format!("\\c {chapter}\n\\p\n"));
        for verse in 1..=30 {
            source.push_str(&format!(
                "\\v {verse} In the beginning God created the heaven and the earth, \
                 and the earth was without form and void.\n"
            ));
        }
        chapter += 1;
    }

    let mut session = Session::new(source);
    let chunks = session.chunks().len();
    let _ = session.content(); // parse everything once, as opening the file would

    let middle = chunks / 2;
    let at = session.chunks()[middle].range().start + 30;

    let started = Instant::now();
    let applied = session.edit(ByteSpan::new(at, at), "x").expect("edit");
    let _ = session.chunk_content(middle);
    let _ = session.chunk_diagnostics(middle);
    let elapsed = started.elapsed();

    // Printed rather than merely asserted: a budget met by 100x and a budget
    // met by 5% call for different decisions later, and the number is free.
    eprintln!(
        "reparse: {elapsed:?} — {} bytes of {} MB, {chunks} chunks",
        applied.invalidated_bytes,
        session.source().len() / 1024 / 1024,
    );

    assert_eq!(
        applied.invalidated, 1,
        "more than one chunk was invalidated"
    );
    assert!(
        elapsed.as_millis() < 15,
        "reparse took {elapsed:?} over {chunks} chunks ({} bytes in the dirty chunk); \
         budget is 15 ms",
        applied.invalidated_bytes
    );
}

/// The comparison that says whether chunking earned its complexity.
#[test]
#[cfg(not(debug_assertions))]
fn chunked_reparse_beats_reparsing_the_whole_document() {
    use std::time::Instant;

    let mut source = String::from("\\id GEN Genesis\n");
    let mut chapter = 1;
    while source.len() < 2 * 1024 * 1024 {
        source.push_str(&format!("\\c {chapter}\n\\p\n"));
        for verse in 1..=30 {
            source.push_str(&format!(
                "\\v {verse} In the beginning God created the heaven and the earth, \
                 and the earth was without form and void.\n"
            ));
        }
        chapter += 1;
    }

    let whole_started = Instant::now();
    let document = Document::parse(source.clone());
    let _ = document.content();
    let _ = document.diagnostics();
    let whole = whole_started.elapsed();

    let mut session = Session::new(source);
    let middle = session.chunks().len() / 2;
    let _ = session.content();
    let at = session.chunks()[middle].range().start + 30;

    let incremental_started = Instant::now();
    session.edit(ByteSpan::new(at, at), "x").expect("edit");
    let _ = session.chunk_content(middle);
    let _ = session.chunk_diagnostics(middle);
    let incremental = incremental_started.elapsed();

    eprintln!("whole document: {whole:?}   one chunk: {incremental:?}");
    assert!(
        incremental < whole / 10,
        "chunking bought less than a 10x improvement: {incremental:?} against {whole:?}"
    );
}

// ------------------------------------------------------------ completion ---

/// The context at the position `@` marks, which stands where the `\` is.
fn context_at(marked: &str) -> easy_usfm_core::CompletionContext {
    let at = marked.find('@').expect("mark the position with @");
    Session::new(marked.replace('@', "")).completion_context(at)
}

#[test]
fn a_backslash_at_the_start_of_a_line_is_line_initial() {
    let context = context_at(SRC_LINE_START);
    assert!(context.line_initial);
    assert_eq!(context.inside, None);
}

#[test]
fn indentation_does_not_stop_a_marker_being_line_initial() {
    // Leading whitespace before a marker occurs in hand-edited files, and the
    // marker is still the first thing on the line.
    assert!(context_at(SRC_INDENTED).line_initial);
}

#[test]
fn a_backslash_after_text_is_not_line_initial() {
    assert!(!context_at(SRC_MID_LINE).line_initial);
}

#[test]
fn a_position_inside_a_character_marker_reports_it() {
    // What decides whether the completion needs the `+` nesting prefix.
    assert_eq!(context_at(SRC_INSIDE_BD).inside.as_deref(), Some("bd"));
}

#[test]
fn a_position_outside_every_character_marker_reports_none() {
    assert_eq!(context_at(SRC_AFTER_BD).inside, None);
}

#[test]
fn the_innermost_marker_wins() {
    assert_eq!(context_at(SRC_NESTED).inside.as_deref(), Some("it"));
}

// Raw strings, so the markers read as they do in a file. `@` stands where the
// backslash being completed is.
const SRC_LINE_START: &str = r"\id GEN
\c 1
@\p
\v 1 a
";
const SRC_INDENTED: &str = r"\id GEN
\c 1
\p
   @\q1 a
";
const SRC_MID_LINE: &str = r"\id GEN
\c 1
\p
\v 1 some text @\bd
";
const SRC_INSIDE_BD: &str = r"\id GEN
\c 1
\p
\v 1 \bd bold @\bd*
";
const SRC_AFTER_BD: &str = r"\id GEN
\c 1
\p
\v 1 \bd bold\bd* plain @
";
const SRC_NESTED: &str = r"\id GEN
\c 1
\p
\v 1 \bd b \+it i @\+it*\bd*
";
