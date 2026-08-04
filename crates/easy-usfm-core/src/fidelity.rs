//! The fidelity envelope — the byte-level properties an editor destroys by
//! default.
//!
//! FILE-FIDELITY §1. A file's byte-order mark, its line-ending style, and
//! whether it ends with a newline are all invisible on screen and all
//! destroyed by a naive load-and-save cycle. They are captured here at load,
//! held **outside** the editor buffer, and reapplied at serialization.
//!
//! Held outside because there is nowhere else to hold them: CodeMirror
//! normalizes line endings on load whatever you do, so by the time text is in
//! a buffer the information is already gone. Capturing separately is not
//! belt-and-braces, it is the only workable response.
//!
//! # This is what M1 is for
//!
//! ROADMAP Part 2 puts file safety before features because these are
//! properties "users cannot verify for themselves and cannot forgive". Someone
//! whose CRLF file silently becomes LF discovers it as a diff touching every
//! line of a translation they have worked on for a year. The round-trip tests
//! are therefore about bytes, not about text.
//!
//! # Not here
//!
//! *Normalization form* is deliberately absent. It is not reapplied, because
//! the buffer holds the original bytes and never normalizes; the detected form
//! is reported and diagnosed instead (UNICODE §4).
//!
//! *Filesystem metadata* — canonical path, symlink status, permissions,
//! mtime — belongs to the same envelope in FILE-FIDELITY §1 but is captured
//! where the filesystem is touched, with the save ladder (P1.5–P1.7). What is
//! here is everything derivable from the bytes alone, which is also everything
//! the web build can obtain (P2.12).

use std::fmt;

/// A line terminator.
///
/// Deserializable as well as serializable, and it is the only part of the
/// envelope that is. The editor is the only thing that knows how a transaction
/// moved the lines, so the per-line array has to come back from it (P1.4);
/// everything else about the envelope stays on the shell's side of the
/// boundary and is never handed to the interface to give back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Eol {
    /// `\n` — Unix, and what the editor buffer always uses internally.
    Lf,
    /// `\r\n` — Windows, and most Paratext output.
    Crlf,
    /// `\r` — classic Mac. Rare, and still found in older translation files.
    Cr,
}

impl Eol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// The name shown in the status bar (PRODUCT §5).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::Crlf => "CRLF",
            Self::Cr => "CR",
        }
    }
}

impl fmt::Display for Eol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// How a file terminates its lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineEndings {
    Uniform(Eol),
    /// A file that disagrees with itself.
    ///
    /// One entry per terminator, in order, so an unmodified line can keep the
    /// terminator it had. FILE-FIDELITY §1 states the rule most designs leave
    /// undefined: *unmodified lines keep their original terminator, a new line
    /// inherits the terminator of the line it was split from, and mixed files
    /// are never silently normalized.* Carrying that array through edits is
    /// P1.4; carrying it through a load and save is here.
    Mixed {
        per_line: Vec<Eol>,
        dominant: Eol,
    },
}

impl LineEndings {
    /// The terminator to use for a line the file has no record of — a line
    /// added since it was loaded.
    pub fn dominant(&self) -> Eol {
        match self {
            Self::Uniform(eol) => *eol,
            Self::Mixed { dominant, .. } => *dominant,
        }
    }

    pub fn is_mixed(&self) -> bool {
        matches!(self, Self::Mixed { .. })
    }

    /// The terminator for the line at `index`, counting from zero.
    fn at(&self, index: usize) -> Eol {
        match self {
            Self::Uniform(eol) => *eol,
            Self::Mixed { per_line, dominant } => per_line.get(index).copied().unwrap_or(*dominant),
        }
    }
}

impl fmt::Display for LineEndings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uniform(eol) => write!(f, "{eol}"),
            Self::Mixed { dominant, .. } => write!(f, "Mixed ({dominant})"),
        }
    }
}

/// Why a file could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The bytes are not valid UTF-8.
    ///
    /// Refused rather than decoded lossily, and the choice is forced: lossy
    /// decoding replaces each bad byte with U+FFFD, which cannot be turned
    /// back into the original byte. Saving would then write a file different
    /// from the one opened, in a way the user never saw and never asked for.
    /// Refusing to open is the only option that keeps ADR-003's promise.
    ///
    /// USFM 3.x is UTF-8 by specification; the project's own documents do not
    /// state this policy, and it is stated here because something had to.
    NotUtf8 { offset: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotUtf8 { offset } => write!(
                f,
                "not valid UTF-8 at byte {offset}; USFM files are UTF-8, and \
                 opening this one would mean changing it"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Everything about a file's bytes that the text alone does not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFidelity {
    pub bom: bool,
    pub eol: LineEndings,
    pub final_newline: bool,
    /// blake3 of the exact bytes read.
    ///
    /// What makes "has this file changed underneath us" answerable (P4.3) and
    /// what lets a save be skipped when nothing changed — FILE-FIDELITY §1: a
    /// clean document's Save is a no-op and does not touch the file.
    pub original_hash: [u8; 32],
    pub len: u64,
}

/// A loaded file: its envelope, and the text the editor gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub fidelity: FileFidelity,
    /// BOM removed and every terminator turned into `\n`.
    ///
    /// The editor uses one separator so change mapping is unambiguous
    /// (`EditorState.lineSeparator.of("\n")`). Everything else lives in the
    /// envelope.
    pub text: String,
}

impl FileFidelity {
    /// Reads the envelope off a file's bytes, and returns the text with it
    /// removed.
    pub fn capture(bytes: &[u8]) -> Result<Loaded, DecodeError> {
        let original_hash = *blake3::hash(bytes).as_bytes();
        let len = bytes.len() as u64;

        let bom = bytes.starts_with(&BOM);
        let body = if bom { &bytes[BOM.len()..] } else { bytes };

        let raw = std::str::from_utf8(body).map_err(|error| DecodeError::NotUtf8 {
            offset: error.valid_up_to() + if bom { BOM.len() } else { 0 },
        })?;

        let (text, terminators) = normalize(raw);
        let final_newline = text.ends_with('\n');
        let eol = classify(&terminators);

        Ok(Loaded {
            fidelity: Self {
                bom,
                eol,
                final_newline,
                original_hash,
                len,
            },
            text,
        })
    }

    /// Puts the envelope back on.
    ///
    /// FILE-FIDELITY §1:
    ///
    /// ```text
    /// bytes = (bom ? EF BB BF : "")
    ///       + join(lines, per-line terminators)
    ///       + (final_newline ? terminator_of_last_line : "")
    /// ```
    ///
    /// `final_newline` is taken from the text rather than from the stored
    /// flag, because the user may have added or removed the trailing newline
    /// and that is an edit like any other. The stored flag records what the
    /// file *had*; the text says what it has now.
    pub fn serialize(&self, text: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(text.len() + 8);
        if self.bom {
            out.extend_from_slice(&BOM);
        }

        let ends_with_newline = text.ends_with('\n');
        let body = if ends_with_newline {
            &text[..text.len() - 1]
        } else {
            text
        };

        for (index, line) in body.split('\n').enumerate() {
            if index > 0 {
                out.extend_from_slice(self.eol.at(index - 1).as_str().as_bytes());
            }
            out.extend_from_slice(line.as_bytes());
        }

        if ends_with_newline {
            // The terminator this newline had, which for a uniform file is
            // simply the file's terminator.
            let last = body.split('\n').count() - 1;
            out.extend_from_slice(self.eol.at(last).as_str().as_bytes());
        }

        out
    }

    /// Whether `bytes` are the ones this envelope was captured from.
    ///
    /// The cheap half of external-change detection (P4.3), and what makes a
    /// clean save a no-op.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        bytes.len() as u64 == self.len && *blake3::hash(bytes).as_bytes() == self.original_hash
    }
}

/// Replaces every terminator with `\n`, recording what each one was.
fn normalize(raw: &str) -> (String, Vec<Eol>) {
    let mut text = String::with_capacity(raw.len());
    let mut terminators = Vec::new();
    let bytes = raw.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                // A lone \r is a terminator in its own right, so the two cases
                // have to be told apart rather than \r simply skipped.
                if bytes.get(index + 1) == Some(&b'\n') {
                    terminators.push(Eol::Crlf);
                    index += 2;
                } else {
                    terminators.push(Eol::Cr);
                    index += 1;
                }
                text.push('\n');
            }
            b'\n' => {
                terminators.push(Eol::Lf);
                text.push('\n');
                index += 1;
            }
            _ => {
                // Copy the whole character, not the byte: slicing mid-sequence
                // would corrupt every non-ASCII line in the file.
                let start = index;
                index += 1;
                while index < bytes.len() && !raw.is_char_boundary(index) {
                    index += 1;
                }
                text.push_str(&raw[start..index]);
            }
        }
    }

    (text, terminators)
}

fn classify(terminators: &[Eol]) -> LineEndings {
    let Some(&first) = terminators.first() else {
        // A file with no terminator at all. LF is the least surprising thing
        // to give a line the user adds later.
        return LineEndings::Uniform(Eol::Lf);
    };

    if terminators.iter().all(|eol| *eol == first) {
        return LineEndings::Uniform(first);
    }

    let (mut lf, mut crlf, mut cr) = (0usize, 0usize, 0usize);
    for eol in terminators {
        match eol {
            Eol::Lf => lf += 1,
            Eol::Crlf => crlf += 1,
            Eol::Cr => cr += 1,
        }
    }

    let dominant = if crlf >= lf && crlf >= cr {
        Eol::Crlf
    } else if lf >= cr {
        Eol::Lf
    } else {
        Eol::Cr
    };

    LineEndings::Mixed {
        per_line: terminators.to_vec(),
        dominant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property everything else exists to support.
    fn assert_round_trips(bytes: &[u8]) {
        let loaded = FileFidelity::capture(bytes).expect("valid UTF-8");
        let written = loaded.fidelity.serialize(&loaded.text);
        assert_eq!(
            written, bytes,
            "round trip changed the file\n  in:  {bytes:?}\n  out: {written:?}"
        );
    }

    #[test]
    fn a_plain_lf_file_round_trips() {
        assert_round_trips(b"\\id GEN\n\\c 1\n");
    }

    #[test]
    fn crlf_survives() {
        // The single most common way an editor damages a translation file.
        assert_round_trips(b"\\id GEN\r\n\\c 1\r\n");
    }

    #[test]
    fn a_lone_cr_survives() {
        assert_round_trips(b"\\id GEN\r\\c 1\r");
    }

    #[test]
    fn a_byte_order_mark_survives() {
        assert_round_trips(b"\xEF\xBB\xBF\\id GEN\n");
    }

    #[test]
    fn a_missing_final_newline_stays_missing() {
        // Adding one looks harmless and shows up as a diff on the last line of
        // every file the editor has touched.
        assert_round_trips(b"\\id GEN\n\\c 1");
    }

    #[test]
    fn mixed_line_endings_are_not_silently_normalized() {
        assert_round_trips(b"\\id GEN\r\n\\c 1\n\\p\r\\v 1 text\n");
    }

    #[test]
    fn an_empty_file_round_trips() {
        assert_round_trips(b"");
    }

    #[test]
    fn a_file_of_only_a_byte_order_mark_round_trips() {
        assert_round_trips(b"\xEF\xBB\xBF");
    }

    #[test]
    fn non_ascii_text_round_trips() {
        assert_round_trips("\\v 1 க்ஷேமம் שלום مرحبا \u{1D400}\r\n".as_bytes());
    }

    #[test]
    fn the_envelope_is_read_correctly() {
        let loaded = FileFidelity::capture(b"\xEF\xBB\xBF\\id GEN\r\n\\c 1\r\n").unwrap();

        assert!(loaded.fidelity.bom);
        assert_eq!(loaded.fidelity.eol, LineEndings::Uniform(Eol::Crlf));
        assert!(loaded.fidelity.final_newline);
        assert_eq!(loaded.text, "\\id GEN\n\\c 1\n");
    }

    #[test]
    fn mixed_endings_are_detected_with_a_dominant() {
        let loaded = FileFidelity::capture(b"a\r\nb\r\nc\n").unwrap();

        assert!(loaded.fidelity.eol.is_mixed());
        assert_eq!(loaded.fidelity.eol.dominant(), Eol::Crlf);
        assert_eq!(loaded.text, "a\nb\nc\n");
    }

    #[test]
    fn the_editor_only_ever_sees_line_feeds() {
        // EditorState.lineSeparator.of("\n") -- one separator, so change
        // mapping is unambiguous (FILE-FIDELITY §1).
        let loaded = FileFidelity::capture(b"a\r\nb\rc\n").unwrap();
        assert!(!loaded.text.contains('\r'));
    }

    #[test]
    fn invalid_utf8_is_refused_rather_than_mangled() {
        // Lossy decoding replaces the bad byte with U+FFFD, which cannot be
        // turned back. Saving would then write a different file.
        let error = FileFidelity::capture(b"\\id GEN\n\xFF\xFE bad\n").unwrap_err();
        assert!(
            matches!(error, DecodeError::NotUtf8 { offset: 8 }),
            "{error:?}"
        );
    }

    #[test]
    fn the_hash_identifies_the_exact_bytes() {
        let bytes = b"\\id GEN\r\n";
        let loaded = FileFidelity::capture(bytes).unwrap();

        assert!(loaded.fidelity.matches(bytes));
        // Same text, different bytes: the LF version must not be mistaken for
        // it, or an external-change check would miss a real change.
        assert!(!loaded.fidelity.matches(b"\\id GEN\n"));
    }

    #[test]
    fn editing_away_the_final_newline_is_respected() {
        // The stored flag records what the file had. The text says what it has
        // now, and an edit that removes the trailing newline is an edit.
        let loaded = FileFidelity::capture(b"a\r\nb\r\n").unwrap();
        assert_eq!(loaded.fidelity.serialize("a\nb"), b"a\r\nb");
    }

    #[test]
    fn a_line_added_beyond_the_original_takes_the_dominant_terminator() {
        let loaded = FileFidelity::capture(b"a\r\nb\r\n").unwrap();
        assert_eq!(loaded.fidelity.serialize("a\nb\nc\n"), b"a\r\nb\r\nc\r\n");
    }
}
