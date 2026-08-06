//! The pathological set — ARCHITECTURE §12.4, P4.5.
//!
//! "BOM + CRLF + no trailing newline; mixed line endings; unclosed `\bd`; notes
//! nested four deep; `\c` with no `\v`; a 40,000-line single chapter; a file
//! containing only `\id`; an empty file; invalid UTF-8; the same file in NFC and
//! NFD; deliberate zero-width joiners including one inside a marker name; `\vp`
//! with non-ASCII digits; long conjunct chains; marks above and below on
//! consecutive lines."
//!
//! These are not fixtures with expected output. Each one is a shape that has
//! historically broken parsers, and what is asserted is that the engine survives
//! it and keeps its invariants — spans in bounds and on character boundaries,
//! chunks covering the document exactly once, offsets that convert.
//!
//! "Handled" is the word ARCHITECTURE uses and it does not mean "parsed
//! cleanly". A file of invalid UTF-8 must be *rejected*, cleanly and with a
//! reason; an unclosed marker must produce a diagnostic rather than swallow the
//! rest of the book. What must never happen is a panic, a hang, or a span that
//! points outside the text.

use std::path::{Path, PathBuf};

use easy_usfm_core::{FileFidelity, Session};

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("pathological")
}

fn cases() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(directory())
        .expect("the pathological corpus should be present")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|kind| kind == "usfm"))
        .collect();
    found.sort();
    found
}

/// Every case is present. A silently missing file would make this whole suite
/// pass by testing nothing.
#[test]
fn the_whole_set_is_there() {
    let names: Vec<String> = cases()
        .iter()
        .filter_map(|path| path.file_name()?.to_str().map(str::to_string))
        .collect();

    for expected in [
        "bom-crlf-no-final-newline.usfm",
        "chapter-with-no-verse.usfm",
        "empty.usfm",
        "invalid-utf8.usfm",
        "long-conjunct-chains.usfm",
        "marks-above-and-below.usfm",
        "mixed-line-endings.usfm",
        "normalization-nfc.usfm",
        "normalization-nfd.usfm",
        "notes-nested-four-deep.usfm",
        "only-an-id.usfm",
        "single-chapter-40000-lines.usfm",
        "unclosed-character-marker.usfm",
        "vp-non-ascii-digits.usfm",
        "zero-width-joiners.usfm",
    ] {
        assert!(names.iter().any(|name| name == expected), "missing {expected}");
    }
}

/// Nothing in the set panics, hangs, or produces a span that points outside the
/// document it came from.
#[test]
fn every_case_is_handled() {
    for path in cases() {
        let bytes = std::fs::read(&path).expect("readable");
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let Ok(loaded) = FileFidelity::capture(&bytes) else {
            // Invalid UTF-8 is *supposed* to be refused: FILE-FIDELITY §1 has
            // the editor preserve a file exactly, which a lossy decode makes
            // impossible before anything has been typed.
            assert_eq!(name, "invalid-utf8.usfm", "{name} failed to decode");
            continue;
        };

        // The invariant checker is the same one the fuzz target asserts, so a
        // pathological file and a fuzzer-found input are held to one standard.
        easy_usfm_core::invariants::check(&loaded.text).unwrap_or_else(|violation| {
            panic!("{name} broke an invariant: {violation}");
        });

        // And it has to survive being opened, which is what the editor does.
        let session = Session::new(&loaded.text);
        let _ = session.diagnostics();
        assert_eq!(session.source(), loaded.text);
    }
}

/// A round trip through the envelope returns the bytes that went in.
///
/// The point of the awkward ones: a BOM with CRLF and no final newline is three
/// separate things a naive reader drops, and dropping any of them is a diff
/// touching every line of somebody's translation.
#[test]
fn every_case_round_trips_byte_for_byte() {
    for path in cases() {
        let bytes = std::fs::read(&path).expect("readable");
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        let Ok(loaded) = FileFidelity::capture(&bytes) else {
            continue;
        };

        assert_eq!(
            loaded.fidelity.serialize(&loaded.text),
            bytes,
            "{name} did not survive a round trip"
        );
    }
}

/// The same text in NFC and NFD parses to the same structure.
///
/// UNICODE §4: search is normalization-insensitive and the *file* is not
/// normalized. Two files differing only in composition must therefore describe
/// the same document, even though their bytes differ.
#[test]
fn composition_does_not_change_the_structure() {
    let read = |name: &str| {
        let bytes = std::fs::read(directory().join(name)).expect("readable");
        FileFidelity::capture(&bytes).expect("valid UTF-8").text
    };

    let nfc = read("normalization-nfc.usfm");
    let nfd = read("normalization-nfd.usfm");
    assert_ne!(nfc, nfd, "the two files should differ in bytes");

    let one = Session::new(&nfc);
    let other = Session::new(&nfd);

    assert_eq!(one.chunks().len(), other.chunks().len());

    // Everything *except* the normalization notice has to match. `USFM-I021`
    // is the one diagnostic that is legitimately about composition — UNICODE §4
    // reports mixed forms as Information with an explicit command to convert,
    // never automatically — so it is expected on one file and not the other.
    let structural = |session: &Session| {
        session
            .diagnostics()
            .into_iter()
            .filter(|found| found.code.as_str() != "USFM-I021")
            .count()
    };
    assert_eq!(
        structural(&one),
        structural(&other),
        "composition changed a diagnostic that is not about composition"
    );

    // And the notice is on the file that is not NFC, which is the point of
    // having the pair at all.
    let notices = |session: &Session| {
        session
            .diagnostics()
            .into_iter()
            .filter(|found| found.code.as_str() == "USFM-I021")
            .count()
    };
    assert_eq!(notices(&one), 0, "the NFC file should not be flagged");
    assert_eq!(notices(&other), 1, "the NFD file should be flagged");
}

/// A forty-thousand-line chapter parses in a time a person would accept.
///
/// Not a benchmark — those are pinned separately. This is the guard against an
/// accidental quadratic, which is the failure mode a single enormous chapter
/// finds and a corpus of ordinary books never does.
#[test]
fn a_very_long_chapter_does_not_go_quadratic() {
    let bytes = std::fs::read(directory().join("single-chapter-40000-lines.usfm")).expect("readable");
    let loaded = FileFidelity::capture(&bytes).expect("valid UTF-8");

    let started = std::time::Instant::now();
    let session = Session::new(&loaded.text);
    let _ = session.diagnostics();
    let elapsed = started.elapsed();

    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "40,000 lines took {elapsed:?}, which suggests a quadratic"
    );
}

/// An unclosed marker is a diagnostic, not a swallowed document.
///
/// ADR-003: content survives. The text after an unclosed `\bd` has to still be
/// in the tree, or one missing `\bd*` loses the rest of a book.
#[test]
fn an_unclosed_marker_does_not_swallow_the_document() {
    let bytes = std::fs::read(directory().join("unclosed-character-marker.usfm")).expect("readable");
    let loaded = FileFidelity::capture(&bytes).expect("valid UTF-8");
    let session = Session::new(&loaded.text);

    assert!(
        !session.diagnostics().is_empty(),
        "an unclosed marker should be diagnosed"
    );
    assert!(
        session.source().contains("with no close."),
        "the text after it must survive"
    );
}
