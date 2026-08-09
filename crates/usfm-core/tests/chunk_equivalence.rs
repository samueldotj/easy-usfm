//! Chunk-boundary equivalence — P0.5, ARCHITECTURE §12.2.
//!
//! A chapter parsed in isolation must produce the same tree as that chapter
//! parsed inside the whole document. If it does not, the incremental session
//! is quietly telling the user something different from what the file says,
//! and the only symptom is a preview that disagrees with the source in a way
//! neither one explains.
//!
//! This is the check that makes P0.4 trustworthy. P0.4's own tests establish
//! that chunking does what it intends; this establishes that what it intends
//! is indistinguishable from not chunking at all — across every committed
//! corpus file, in twelve scripts, rather than across examples chosen by the
//! person who wrote the chunker.

use std::path::{Path, PathBuf};

use usfm_core::{ByteSpan, Document, Node, NodeKind, Session};

// ------------------------------------------------------------ comparison ---

/// One node, flattened to what equivalence is actually about.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shape {
    depth: usize,
    kind: NodeKind,
    marker: Option<String>,
    attributes: Vec<(String, String)>,
    /// Text is compared verbatim. Whitespace handling at a chunk seam is
    /// exactly the sort of thing that would otherwise drift unnoticed.
    text: Option<String>,
    /// `None` for text and optional breaks, which the parser records without
    /// a source location at all.
    span: Option<ByteSpan>,
}

fn flatten(nodes: &[Node], depth: usize, into: &mut Vec<Shape>) {
    for node in nodes {
        into.push(Shape {
            depth,
            kind: node.kind,
            marker: node.marker.as_ref().map(|m| m.as_str().to_string()),
            attributes: node
                .attributes
                .iter()
                .map(|a| (a.key.clone(), a.value.clone()))
                .collect(),
            text: node.text.clone(),
            span: node.span.clone(),
        });
        flatten(&node.children, depth + 1, into);
    }
}

fn shapes(nodes: &[Node]) -> Vec<Shape> {
    let mut out = Vec::new();
    flatten(nodes, 0, &mut out);
    out
}

/// Compares the two parses, returning a readable account of the first
/// disagreement.
fn difference(source: &str) -> Option<String> {
    let chunked = shapes(&Session::new(source.to_string()).content());
    let document = Document::parse(source.to_string());
    let whole = shapes(document.content());

    if chunked == whole {
        return None;
    }

    let at = chunked
        .iter()
        .zip(&whole)
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| chunked.len().min(whole.len()));

    let context = |shapes: &[Shape]| -> String {
        shapes
            .iter()
            .skip(at.saturating_sub(2))
            .take(5)
            .map(|shape| format!("      {shape:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Some(format!(
        "  {} nodes chunked, {} whole; first difference at {at}\n\
         \x20   chunked:\n{}\n\
         \x20   whole:\n{}",
        chunked.len(),
        whole.len(),
        context(&chunked),
        context(&whole),
    ))
}

fn assert_equivalent(label: &str, source: &str) {
    if let Some(report) = difference(source) {
        panic!("chunked parse disagrees with whole-document parse — {label}\n{report}");
    }
}

// ------------------------------------------------------- the five cases ---
//
// ARCHITECTURE §12.2 names five edits worth testing on their own, because
// each one moves a chunk boundary and the corpus contains no edits at all.

const BOOK: &str = "\\id GEN Genesis\n\
                    \\h Genesis\n\
                    \\c 1\n\\p\n\\v 1 In the beginning.\n\\v 2 And the earth.\n\
                    \\c 2\n\\p\n\\v 1 Thus the heavens.\n\\v 2 And on the seventh day.\n\
                    \\c 3\n\\p\n\\v 1 Now the serpent.\n";

/// Applies an edit through the session, then checks the result parses the same
/// way from scratch — which is what "the incremental state is still correct"
/// means.
fn assert_edit_preserves_equivalence(label: &str, at: usize, remove: usize, insert: &str) {
    let mut session = Session::new(BOOK.to_string());
    let _ = session.content(); // parse everything first, so caches are live

    session
        .edit(ByteSpan::new(at, at + remove), insert)
        .unwrap_or_else(|error| panic!("{label}: edit refused: {error}"));

    let after = session.source().to_string();
    let incremental = shapes(&session.content());
    let from_scratch = shapes(&Session::new(after.clone()).content());
    let document = Document::parse(after);
    let whole = shapes(document.content());

    assert_eq!(
        incremental, from_scratch,
        "{label}: the incrementally updated tree differs from a fresh chunked parse"
    );
    assert_eq!(
        incremental, whole,
        "{label}: the incrementally updated tree differs from a whole-document parse"
    );
}

#[test]
fn case_1_inserting_a_chapter_marker() {
    let at = BOOK.find("\\v 2 And the earth").expect("verse");
    assert_edit_preserves_equivalence("inserting \\c", at, 0, "\\c 4\n\\p\n");
}

#[test]
fn case_2_deleting_a_chapter_marker() {
    let at = BOOK.find("\\c 2\n").expect("chapter 2");
    assert_edit_preserves_equivalence("deleting \\c", at, "\\c 2\n".len(), "");
}

#[test]
fn case_3_splitting_a_chapter() {
    // Mid-verse, so the split lands somewhere no boundary rule was written
    // with in mind.
    let at = BOOK.find("Thus the heavens").expect("verse text") + 5;
    assert_edit_preserves_equivalence("splitting a chapter", at, 0, "\n\\c 9\n\\p\n\\v 1 ");
}

#[test]
fn case_4_editing_the_header_chunk() {
    let at = BOOK.find("Genesis").expect("book name");
    assert_edit_preserves_equivalence("editing the header", at, "Genesis".len(), "Exodus");
}

#[test]
fn case_5_editing_at_the_exact_boundary() {
    // The first byte of a chapter marker: the position where the chunk that
    // owns the edit and the chunk that owns the boundary are different chunks.
    let at = BOOK.find("\\c 2\n").expect("chapter 2");
    assert_edit_preserves_equivalence("at the boundary, inserting before", at, 0, "\\p\n");
    assert_edit_preserves_equivalence("at the boundary, overwriting", at, 1, "\\");
    assert_edit_preserves_equivalence("at the boundary, deleting the newline", at - 1, 1, "");
}

// ------------------------------------------------------------- pathology ---

#[test]
fn equivalence_holds_for_shapes_that_stress_the_boundary() {
    let cases: &[(&str, &str)] = &[
        ("empty", ""),
        ("header only", "\\id GEN\n\\h Genesis\n"),
        ("chapter with no verses", "\\id GEN\n\\c 1\n\\c 2\n"),
        ("no header chunk", "\\c 1\n\\p\n\\v 1 text\n"),
        ("no trailing newline", "\\id GEN\n\\c 1\n\\p\n\\v 1 text"),
        ("crlf", "\\id GEN\r\n\\c 1\r\n\\p\r\n\\v 1 text\r\n"),
        ("blank lines between chapters", "\\id GEN\n\\c 1\n\n\n\\c 2\n\\p\n\\v 1 a\n"),
        (
            "unclosed marker crossing to the next chapter",
            "\\id GEN\n\\c 1\n\\p\n\\v 1 \\bd unclosed\n\\c 2\n\\p\n\\v 1 next\n",
        ),
        (
            "note left open at a chapter boundary",
            "\\id GEN\n\\c 1\n\\p\n\\v 1 \\f + \\ft open\n\\c 2\n\\p\n\\v 1 next\n",
        ),
        (
            "milestone spanning a chapter boundary",
            "\\id GEN\n\\c 1\n\\p\n\\v 1 \\qt-s |who=\"X\"\\*quoted\n\\c 2\n\\p\n\\v 1 \\qt-e\\*\n",
        ),
        (
            "markers that only look like chapters",
            "\\id GEN\n\\c 1\n\\cl Psalm\n\\cp A\n\\ca 2\\ca*\n\\p\n\\v 1 text\n",
        ),
        (
            "mixed scripts across chapters",
            "\\id GEN\n\\c 1\n\\p\n\\v 1 க்ஷேமம்\n\\c 2\n\\p\n\\v 1 שלום\n\\c 3\n\\p\n\\v 1 \u{1D400}\n",
        ),
    ];

    for (label, source) in cases {
        assert_equivalent(label, source);
    }
}

// ---------------------------------------------------------------- corpus ---

/// Both extensions, in either case. The BSB files are `.SFM`, and a filter
/// that only matched `.usfm` skipped all ten of them without saying so.
fn is_usfm(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("usfm") || extension.eq_ignore_ascii_case("sfm")
        })
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("core")
}

/// The assertion P0.5 actually exists for.
///
/// Fixtures test what their author thought of; 200 real files in twelve
/// scripts test what nobody thought of. Every disagreement is reported rather
/// than just the first, because a chunking bug usually shows up in a family of
/// files at once and the shape of the family is the diagnosis.
#[test]
fn chunked_parsing_agrees_with_whole_document_parsing_across_the_corpus() {
    let directory = corpus_dir();

    let mut files: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("reading {}: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_usfm(path))
        .collect();
    files.sort();

    assert!(
        files.len() >= 100,
        "expected the committed corpus, found {} files in {}",
        files.len(),
        directory.display()
    );

    let mut failures = Vec::new();
    let mut chunks_total = 0usize;

    for path in files.iter() {
        let raw = std::fs::read(path).expect("corpus file is readable");
        let Ok(source) = String::from_utf8(raw) else {
            continue; // invalid UTF-8 is P0.11's problem, not this one
        };

        chunks_total += Session::new(source.clone()).chunks().len();

        if let Some(report) = difference(&source) {
            failures.push(format!(
                "{}\n{report}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
    }

    eprintln!(
        "chunk equivalence: {} files, {chunks_total} chunks",
        files.len()
    );

    assert!(
        failures.is_empty(),
        "{} of {} corpus files parse differently when chunked:\n\n{}",
        failures.len(),
        files.len(),
        failures.join("\n\n")
    );
}

/// Every chunk boundary must fall on a character boundary and tile the file
/// exactly. A gap loses text and an overlap duplicates it, and both are
/// invisible until someone edits near the seam.
#[test]
fn chunks_tile_every_corpus_file_exactly() {
    let mut checked = 0usize;

    for entry in std::fs::read_dir(corpus_dir()).expect("corpus directory") {
        let path = entry.expect("directory entry").path();
        if !is_usfm(&path) {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };

        let session = Session::new(source.clone());
        let mut expected = 0usize;

        for chunk in session.chunks() {
            let range = chunk.range();
            assert_eq!(
                range.start,
                expected,
                "gap or overlap in {}",
                path.display()
            );
            assert!(
                source.is_char_boundary(range.start) && source.is_char_boundary(range.end),
                "chunk boundary falls inside a character in {}",
                path.display()
            );
            expected = range.end;
        }

        assert_eq!(
            expected,
            source.len(),
            "chunks do not cover {}",
            path.display()
        );
        checked += 1;
    }

    assert!(checked >= 100, "only {checked} files were checked");
}
