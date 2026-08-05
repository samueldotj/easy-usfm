//! Open documents, and the commands the interface drives them with.
//!
//! # The envelope stays on this side
//!
//! A document's fidelity envelope — its byte-order mark, its per-line
//! terminators, the hash of the bytes it was read from — is held here and
//! never sent to the interface. Two reasons, and the second is the real one.
//!
//! It would be a large payload to carry per document, and more importantly it
//! would become something the interface could get wrong. The envelope is the
//! thing standing between a translator and a diff touching every line of their
//! file; it is not a value to be round-tripped through a webview and handed
//! back on save. What crosses is text, and a summary for the status bar.
//!
//! The one exception is the per-line terminator array, which *must* be updated
//! by the editor because only the editor knows how each transaction moved the
//! lines (P1.4). It is sent back with the save and applied here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use easy_usfm_core::{Eol, FileFidelity, LineEndings};
use serde::{Deserialize, Serialize};

use crate::fs::{FileSystem, RealFs};
use crate::save::{save, SaveError};

/// A document the interface is holding open.
struct Open {
    path: Option<PathBuf>,
    fidelity: FileFidelity,
}

/// Every open document. One window today; the map is what makes more than one
/// possible without revisiting this.
#[derive(Default)]
pub struct Documents(Mutex<HashMap<u64, Open>>);

/// What the interface is given when a document opens.
#[derive(Debug, Serialize)]
pub struct Opened {
    pub id: u64,
    pub path: Option<String>,
    /// Terminators normalized to `\n`, byte-order mark removed.
    pub text: String,
    pub summary: Summary,
    /// One entry per newline, so the editor can carry them through edits.
    pub eols: Vec<Eol>,
}

/// What the status bar shows (PRODUCT §5).
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub encoding: &'static str,
    pub eol: String,
    pub bom: bool,
    pub final_newline: bool,
    pub mixed_eol: bool,
}

impl Summary {
    fn of(fidelity: &FileFidelity) -> Self {
        Self {
            // The only encoding the engine accepts; stated rather than
            // detected, because accepting others would break byte fidelity.
            encoding: "UTF-8",
            eol: fidelity.eol.to_string(),
            bom: fidelity.bom,
            final_newline: fidelity.final_newline,
            mixed_eol: fidelity.eol.is_mixed(),
        }
    }
}

/// What a save reports back.
#[derive(Debug, Serialize)]
pub struct SaveReport {
    pub path: String,
    /// Set when the save took the slower rung, so the status bar can say why
    /// rather than leaving the delay unexplained (FILE-FIDELITY §2).
    pub reason: Option<String>,
    pub summary: Summary,
}

#[derive(Debug, Deserialize)]
pub struct SaveRequest {
    pub id: u64,
    pub text: String,
    /// The editor's current per-line terminators. Empty means "unchanged",
    /// which is what a document that has not been edited sends.
    #[serde(default)]
    pub eols: Vec<Eol>,
}

fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// The template a new document starts from.
///
/// `\id` is required and a file without one is an error, so an empty buffer
/// would greet every new document with a diagnostic. Starting from the
/// smallest valid document is friendlier and just as honest.
use easy_usfm_core::NEW_DOCUMENT;

impl Documents {
    fn insert(&self, path: Option<PathBuf>, fidelity: FileFidelity) -> u64 {
        let id = next_id();
        if let Ok(mut open) = self.0.lock() {
            open.insert(id, Open { path, fidelity });
        }
        id
    }
}

// ------------------------------------------------------------- commands ---

#[tauri::command]
pub fn new_document(documents: tauri::State<'_, Documents>) -> Result<Opened, String> {
    let loaded = FileFidelity::capture(NEW_DOCUMENT.as_bytes()).map_err(|e| e.to_string())?;
    let summary = Summary::of(&loaded.fidelity);
    let eols = terminators(&loaded.fidelity, loaded.text.matches('\n').count());
    let id = documents.insert(None, loaded.fidelity);

    Ok(Opened {
        id,
        path: None,
        text: loaded.text,
        summary,
        eols,
    })
}

#[tauri::command]
pub fn open_document(
    path: String,
    documents: tauri::State<'_, Documents>,
) -> Result<Opened, String> {
    let path = PathBuf::from(path);
    let bytes = RealFs
        .read(&path)
        .map_err(|error| format!("{}: {error}", path.display()))?;

    // Refused rather than decoded lossily -- opening a file we cannot write
    // back unchanged is the one thing this application must not do.
    let loaded = FileFidelity::capture(&bytes).map_err(|error| error.to_string())?;

    let summary = Summary::of(&loaded.fidelity);
    let eols = terminators(&loaded.fidelity, loaded.text.matches('\n').count());
    let id = documents.insert(Some(path.clone()), loaded.fidelity);

    Ok(Opened {
        id,
        path: Some(path.to_string_lossy().to_string()),
        text: loaded.text,
        summary,
        eols,
    })
}

#[tauri::command]
pub fn save_document(
    request: SaveRequest,
    path: Option<String>,
    documents: tauri::State<'_, Documents>,
) -> Result<SaveReport, String> {
    let mut open = documents.0.lock().map_err(|_| "document store poisoned")?;
    let document = open
        .get_mut(&request.id)
        .ok_or_else(|| "no such document".to_string())?;

    // Save As supplies a path; Save uses the one the document already has. A
    // document with neither has never been saved and cannot be saved silently.
    let target = match path.as_deref() {
        Some(path) => PathBuf::from(path),
        None => document
            .path
            .clone()
            .ok_or_else(|| "this document has no path yet".to_string())?,
    };

    // The editor's terminators, if it sent any. Only the editor knows how each
    // transaction moved the lines.
    if !request.eols.is_empty() {
        let dominant = document.fidelity.eol.dominant();
        document.fidelity.eol = if request.eols.iter().all(|eol| *eol == request.eols[0]) {
            LineEndings::Uniform(request.eols[0])
        } else {
            LineEndings::Mixed {
                per_line: request.eols.clone(),
                dominant,
            }
        };
    }

    let bytes = document.fidelity.serialize(&request.text);

    let saved = save(&RealFs, &target, &bytes).map_err(|error| match &error {
        // Rung 3. The interface offers Save As rather than reporting a failure
        // the user cannot act on.
        SaveError::ReadOnly { .. } => format!("READONLY:{error}"),
        SaveError::Failed { .. } => error.to_string(),
    })?;

    // The document now *is* what was written, so the envelope is recaptured
    // from the bytes that reached the disk rather than assumed.
    if let Ok(reloaded) = FileFidelity::capture(&bytes) {
        document.fidelity = reloaded.fidelity;
    }
    document.path = Some(saved.path.clone());

    Ok(SaveReport {
        path: saved.path.to_string_lossy().to_string(),
        reason: saved.reason.map(|reason| {
            match reason {
                crate::save::CopyBackReason::HardLinked => "linked file",
                crate::save::CopyBackReason::SyncRoot => "cloud folder",
                crate::save::CopyBackReason::RenameFailed => "in-place write",
            }
            .to_string()
        }),
        summary: Summary::of(&document.fidelity),
    })
}

#[tauri::command]
pub fn close_document(id: u64, documents: tauri::State<'_, Documents>) {
    if let Ok(mut open) = documents.0.lock() {
        open.remove(&id);
    }
}

/// The per-line terminators, expanded to one entry per newline.
///
/// Delegates to the core, which is where the web shell reads it from too --
/// two copies of this is two chances for a file's line endings to come back
/// different depending on which build opened it.
fn terminators(fidelity: &FileFidelity, newlines: usize) -> Vec<Eol> {
    fidelity.eol.per_line(newlines)
}

/// Whether the path looks like a USFM file, for the dialog's filter.
pub fn is_usfm(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("usfm") || e.eq_ignore_ascii_case("sfm"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_document_is_valid_usfm() {
        // An empty buffer would greet every new document with a missing-\id
        // error, which is a poor way to start.
        let parsed = easy_usfm_core::Document::parse(NEW_DOCUMENT.to_string());
        assert!(
            !parsed
                .diagnostics()
                .iter()
                .any(|d| d.code == easy_usfm_core::DiagnosticCode::MissingIdMarker),
            "{:?}",
            parsed.diagnostics()
        );
    }

    #[test]
    fn terminators_are_expanded_to_one_per_newline() {
        let loaded = FileFidelity::capture(b"a\r\nb\r\nc\r\n").unwrap();
        assert_eq!(terminators(&loaded.fidelity, 3), vec![Eol::Crlf; 3]);
    }

    #[test]
    fn a_mixed_document_keeps_its_own_and_pads_with_the_dominant() {
        let loaded = FileFidelity::capture(b"a\r\nb\nc\r\n").unwrap();
        let expanded = terminators(&loaded.fidelity, 5);

        assert_eq!(&expanded[..3], &[Eol::Crlf, Eol::Lf, Eol::Crlf]);
        // Lines added since the file was read take the dominant terminator.
        assert_eq!(&expanded[3..], &[Eol::Crlf, Eol::Crlf]);
    }

    #[test]
    fn usfm_files_are_recognised_in_either_case() {
        assert!(is_usfm(Path::new("gen.usfm")));
        assert!(is_usfm(Path::new("01GENBSB.SFM")));
        assert!(!is_usfm(Path::new("notes.txt")));
    }
}
