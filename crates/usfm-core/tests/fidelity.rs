//! Byte-exact round-tripping — P1.3.
//!
//! FILE-FIDELITY opens by saying file safety is "the property users cannot
//! verify for themselves and cannot forgive". These tests are the part that
//! can be verified, and they are written about bytes rather than about text,
//! because every property at stake here is invisible once the file is text.
//!
//! Three levels, in increasing order of how much they prove:
//!
//! 1. Fixtures — the cases someone thought of.
//! 2. A generated property over every combination of BOM, terminator, and
//!    trailing newline — the cases nobody thought of.
//! 3. Every committed corpus file — real published Scripture, in twelve
//!    scripts, none of it written to make us look good.
//!
//! The full T1–T3 suite over the corpus is P1.9; level 3 here is the part of
//! it that the fidelity envelope alone can carry.

use std::path::{Path, PathBuf};

use proptest::prelude::*;
use usfm_core::{Eol, FileFidelity};

fn round_trips(bytes: &[u8]) -> Result<(), String> {
    let loaded = FileFidelity::capture(bytes).map_err(|error| error.to_string())?;
    let written = loaded.fidelity.serialize(&loaded.text);

    if written == bytes {
        Ok(())
    } else {
        Err(format!(
            "changed on round trip\n  in:  {:?}\n  out: {:?}",
            String::from_utf8_lossy(bytes),
            String::from_utf8_lossy(&written)
        ))
    }
}

// ----------------------------------------------------------- the matrix ---

/// Every combination of the three properties, over content chosen to be
/// awkward.
///
/// Exhaustive rather than sampled: there are only twenty-four combinations and
/// the whole point is that none of them is special.
#[test]
fn every_combination_of_bom_terminator_and_final_newline_round_trips() {
    let bodies: &[&str] = &[
        "",
        "\\id GEN",
        "\\id GEN|\\c 1|\\p|\\v 1 text",
        // Non-ASCII, so a byte-wise implementation that slices mid-character
        // fails here rather than in production.
        "\\v 1 க்ஷேமம்|\\v 2 שלום|\\v 3 \u{1D400}",
        // Blank lines, which produce consecutive terminators.
        "a||b",
    ];

    for &bom in &[false, true] {
        for eol in [Eol::Lf, Eol::Crlf, Eol::Cr] {
            for &trailing in &[false, true] {
                for body in bodies {
                    let mut text = body.replace('|', eol.as_str());
                    if trailing && !text.is_empty() {
                        text.push_str(eol.as_str());
                    }

                    let mut bytes = Vec::new();
                    if bom {
                        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
                    }
                    bytes.extend_from_slice(text.as_bytes());

                    round_trips(&bytes).unwrap_or_else(|error| {
                        panic!("bom={bom} eol={eol} trailing={trailing}: {error}")
                    });
                }
            }
        }
    }
}

// --------------------------------------------------------- the property ---

/// Lines built from pieces that have caused trouble elsewhere in the project.
fn line() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("\\id GEN".to_string()),
            Just("\\v 1 text".to_string()),
            Just("க்ஷ".to_string()),
            Just("שלום".to_string()),
            Just("\u{1D400}".to_string()),
            Just("e\u{301}".to_string()),
            Just("".to_string()),
            Just("   ".to_string()),
        ],
        0..4,
    )
    .prop_map(|parts| parts.join(" "))
}

fn eol() -> impl Strategy<Value = Eol> {
    prop_oneof![Just(Eol::Lf), Just(Eol::Crlf), Just(Eol::Cr)]
}

proptest! {
    /// Any file built from any mixture of terminators round-trips exactly.
    ///
    /// Mixed endings are generated deliberately. FILE-FIDELITY §1 calls the
    /// rule for them "the rule most designs leave undefined", and a file that
    /// disagrees with itself is exactly where a normalize-on-load
    /// implementation looks correct until someone opens one.
    #[test]
    fn any_mixture_of_line_endings_round_trips(
        bom in any::<bool>(),
        lines in prop::collection::vec((line(), eol()), 0..12),
        trailing in any::<bool>(),
    ) {
        let mut bytes = Vec::new();
        if bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }

        for (index, (text, terminator)) in lines.iter().enumerate() {
            bytes.extend_from_slice(text.as_bytes());
            let last = index + 1 == lines.len();
            if !last || trailing {
                bytes.extend_from_slice(terminator.as_str().as_bytes());
            }
        }

        prop_assert!(round_trips(&bytes).is_ok(), "{}", round_trips(&bytes).unwrap_err());
    }

    /// Capturing twice gives the same answer, and the envelope survives being
    /// applied to its own output.
    #[test]
    fn capture_is_idempotent(
        bom in any::<bool>(),
        lines in prop::collection::vec((line(), eol()), 0..8),
    ) {
        let mut bytes = Vec::new();
        if bom {
            bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        for (text, terminator) in &lines {
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(terminator.as_str().as_bytes());
        }

        let first = FileFidelity::capture(&bytes).expect("valid");
        let written = first.fidelity.serialize(&first.text);
        let second = FileFidelity::capture(&written).expect("valid");

        prop_assert_eq!(first.fidelity, second.fidelity);
        prop_assert_eq!(first.text, second.text);
    }
}

// ------------------------------------------------------------- the corpus ---

fn corpus_files() -> Vec<PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("corpus")
        .join("core");

    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("usfm") || e.eq_ignore_ascii_case("sfm"))
        })
        .collect();
    files.sort();
    files
}

/// Every committed file survives a load and a save unchanged, byte for byte.
#[test]
fn every_corpus_file_round_trips_byte_for_byte() {
    let files = corpus_files();
    assert!(files.len() >= 100, "expected the committed corpus");

    let mut failures = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).expect("corpus file is readable");
        if let Err(error) = round_trips(&bytes) {
            failures.push(format!(
                "{}: {error}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} corpus files changed on a load-and-save cycle:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
}

/// What the committed corpus actually exercises.
///
/// Printed rather than asserted, because it is a fact about the corpus rather
/// than about this code — and it is the evidence behind P0.1's open question,
/// that published Scripture carries none of the messy encodings FILE-FIDELITY
/// exists to protect.
#[test]
fn report_what_the_corpus_covers() {
    let (mut bom, mut crlf, mut cr, mut mixed, mut no_final) = (0, 0, 0, 0, 0);
    let files = corpus_files();

    for path in &files {
        let bytes = std::fs::read(path).expect("readable");
        let Ok(loaded) = FileFidelity::capture(&bytes) else {
            continue;
        };

        if loaded.fidelity.bom {
            bom += 1;
        }
        if loaded.fidelity.eol.is_mixed() {
            mixed += 1;
        }
        match loaded.fidelity.eol.dominant() {
            Eol::Crlf => crlf += 1,
            Eol::Cr => cr += 1,
            Eol::Lf => {}
        }
        if !loaded.fidelity.final_newline {
            no_final += 1;
        }
    }

    eprintln!(
        "corpus fidelity: {} files — {bom} with a BOM, {crlf} CRLF, {cr} CR, \
         {mixed} mixed, {no_final} without a final newline",
        files.len()
    );
}
