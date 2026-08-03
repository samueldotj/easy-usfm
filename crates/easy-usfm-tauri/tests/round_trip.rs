//! T1, T2, T3 — FILE-FIDELITY §5.1, over the whole corpus.
//!
//! The obvious test is *open, save, compare*, and it is unfalsifiable: §1 makes
//! a clean document's Save a no-op, so it passes on an implementation with no
//! serializer at all. Three tests replace it.
//!
//! ```text
//! T1  Idempotent edit    open → insert → undo → save → compare bytes
//! T2  Save As            open → Save As, no edits → compare bytes
//! T3  Localized edit     open → edit one verse → save → diff
//! ```
//!
//! **T3 is the important one.** T1 and T2 pass on a system that quietly
//! rewrites the whole document identically; T3 cannot, because it asserts what
//! *did not* change.
//!
//! # The corpus cannot carry these on its own
//!
//! Every committed file is LF, without a byte-order mark, and none is mixed —
//! measured in P1.3, and the open question from P0.1. Run against the corpus
//! alone these tests would prove only that LF files survive. So each file is
//! also re-encoded into the forms that published Scripture never carries and
//! that FILE-FIDELITY exists to protect, and the tests run against those too.

use std::path::{Path, PathBuf};

use easy_usfm_core::{Eol, FileFidelity};
use easy_usfm_tauri_lib::fs::RealFs;
use easy_usfm_tauri_lib::save::save;

// ------------------------------------------------------------- fixtures ---

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

/// Re-encodes LF bytes into another shape, without touching the text.
fn re_encode(bytes: &[u8], bom: bool, style: Style) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::new();
    if bom {
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }

    let ends_with_newline = text.ends_with('\n');
    let body = if ends_with_newline {
        &text[..text.len() - 1]
    } else {
        &text[..]
    };

    for (index, line) in body.split('\n').enumerate() {
        if index > 0 {
            out.extend_from_slice(style.at(index - 1).as_str().as_bytes());
        }
        out.extend_from_slice(line.as_bytes());
    }
    if ends_with_newline {
        let last = body.split('\n').count() - 1;
        out.extend_from_slice(style.at(last).as_str().as_bytes());
    }

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Lf,
    Crlf,
    /// Alternating, so the per-line array is genuinely exercised rather than
    /// happening to be uniform.
    Mixed,
}

impl Style {
    fn at(self, index: usize) -> Eol {
        match self {
            Self::Lf => Eol::Lf,
            Self::Crlf => Eol::Crlf,
            Self::Mixed => match index % 3 {
                0 => Eol::Crlf,
                1 => Eol::Lf,
                _ => Eol::Cr,
            },
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::Crlf => "CRLF",
            Self::Mixed => "mixed",
        }
    }
}

/// Each corpus file, in the encodings the corpus itself does not contain.
fn variants(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    vec![
        ("as committed".to_string(), bytes.to_vec()),
        ("BOM".to_string(), re_encode(bytes, true, Style::Lf)),
        ("CRLF".to_string(), re_encode(bytes, false, Style::Crlf)),
        (
            "BOM + CRLF".to_string(),
            re_encode(bytes, true, Style::Crlf),
        ),
        ("mixed".to_string(), re_encode(bytes, false, Style::Mixed)),
    ]
}

/// Splits into lines, each keeping its own terminator, so a comparison can see
/// a changed line ending as a changed line.
fn lines_with_endings(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                let width = if bytes.get(index + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                lines.push(&bytes[start..index + width]);
                index += width;
                start = index;
            }
            b'\n' => {
                lines.push(&bytes[start..=index]);
                index += 1;
                start = index;
            }
            _ => index += 1,
        }
    }
    if start < bytes.len() {
        lines.push(&bytes[start..]);
    }
    lines
}

fn write_and_read(dir: &Path, name: &str, bytes: &[u8]) -> Vec<u8> {
    let path = dir.join(name);
    std::fs::write(&path, b"placeholder").expect("seed the target");
    save(&RealFs, &path, bytes).expect("save succeeds");
    std::fs::read(&path).expect("read back")
}

// --------------------------------------------------------------- T1 & T2 ---

/// T1 — an edit that is undone leaves the file byte-identical.
///
/// The serializer runs, because the document has been touched. That is what
/// makes this different from the naive open-and-save.
#[test]
fn t1_an_undone_edit_saves_byte_identical_bytes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut checked = 0usize;

    for path in corpus_files() {
        let original = std::fs::read(&path).expect("read");

        for (label, bytes) in variants(&original) {
            let loaded = FileFidelity::capture(&bytes).expect("valid UTF-8");

            // Insert, then undo: the text is back where it started, but the
            // document has been through the editor.
            let mut text = loaded.text.clone();
            text.insert(0, 'x');
            text.remove(0);

            let written = loaded.fidelity.serialize(&text);
            let read_back = write_and_read(dir.path(), "t1.usfm", &written);

            assert_eq!(
                read_back,
                bytes,
                "T1 changed {} ({label})",
                path.file_name().unwrap().to_string_lossy()
            );
            checked += 1;
        }
    }

    assert!(checked >= 500, "only {checked} cases ran");
}

/// T2 — Save As with no edits produces the same bytes at the new path.
#[test]
fn t2_save_as_produces_identical_bytes() {
    let dir = tempfile::tempdir().expect("temp dir");

    for path in corpus_files() {
        let original = std::fs::read(&path).expect("read");

        for (label, bytes) in variants(&original) {
            let loaded = FileFidelity::capture(&bytes).expect("valid UTF-8");
            let written = loaded.fidelity.serialize(&loaded.text);

            // A path that does not exist yet, which is what Save As means.
            let target = dir.path().join("t2-new.usfm");
            let _ = std::fs::remove_file(&target);
            save(&RealFs, &target, &written).expect("save as succeeds");

            assert_eq!(
                std::fs::read(&target).expect("read back"),
                bytes,
                "T2 changed {} ({label})",
                path.file_name().unwrap().to_string_lossy()
            );
        }
    }
}

// -------------------------------------------------------------------- T3 ---

/// T3 — editing one verse changes only that verse's line.
///
/// The test that catches accidental whole-document normalization, which is the
/// failure the preservation guarantee actually cares about. Everything else
/// about the file — its byte-order mark, the line endings on every other line,
/// blank lines, and the trailing newline — must come back untouched.
#[test]
fn t3_a_localized_edit_touches_only_that_line() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut edited_files = 0usize;

    for path in corpus_files() {
        let original = std::fs::read(&path).expect("read");

        for (label, bytes) in variants(&original) {
            let loaded = FileFidelity::capture(&bytes).expect("valid UTF-8");

            // The first verse line. Editing a verse rather than an arbitrary
            // line is the point: it is what a translator actually does.
            let lines: Vec<&str> = loaded.text.split('\n').collect();
            let Some(target_line) = lines.iter().position(|line| line.starts_with("\\v ")) else {
                continue;
            };

            let mut edited: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            edited[target_line].push_str(" EDITED");

            let written = loaded.fidelity.serialize(&edited.join("\n"));
            let read_back = write_and_read(dir.path(), "t3.usfm", &written);

            let before = lines_with_endings(&bytes);
            let after = lines_with_endings(&read_back);

            assert_eq!(
                before.len(),
                after.len(),
                "T3 changed the line count of {} ({label})",
                path.file_name().unwrap().to_string_lossy()
            );

            for (index, (was, now)) in before.iter().zip(&after).enumerate() {
                if index == target_line {
                    assert_ne!(was, now, "the edited line did not change");
                    continue;
                }
                assert_eq!(
                    was,
                    now,
                    "T3 touched line {index} of {} ({label}), which was not edited.\n  \
                     was: {:?}\n  now: {:?}",
                    path.file_name().unwrap().to_string_lossy(),
                    String::from_utf8_lossy(was),
                    String::from_utf8_lossy(now)
                );
            }

            // And the envelope itself came back.
            let after_envelope = FileFidelity::capture(&read_back).expect("valid");
            assert_eq!(after_envelope.fidelity.bom, loaded.fidelity.bom);
            assert_eq!(
                after_envelope.fidelity.final_newline,
                loaded.fidelity.final_newline
            );

            edited_files += 1;
        }
    }

    assert!(edited_files >= 500, "only {edited_files} cases ran");
}

/// The encodings the corpus does not contain are the ones being relied on, so
/// the re-encoder itself is checked rather than trusted.
#[test]
fn the_re_encoder_produces_what_it_claims() {
    let source = b"a\nb\nc\n";

    let crlf = re_encode(source, false, Style::Crlf);
    assert_eq!(crlf, b"a\r\nb\r\nc\r\n");

    let bom = re_encode(source, true, Style::Lf);
    assert_eq!(bom, b"\xEF\xBB\xBFa\nb\nc\n");

    let mixed = re_encode(source, false, Style::Mixed);
    assert_eq!(mixed, b"a\r\nb\nc\r");
    assert!(
        FileFidelity::capture(&mixed)
            .unwrap()
            .fidelity
            .eol
            .is_mixed(),
        "the mixed variant is not actually mixed"
    );
    assert_eq!(Style::Mixed.label(), "mixed");
}
