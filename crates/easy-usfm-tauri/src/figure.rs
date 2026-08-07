//! Reading a figure's image, and refusing everything else — SECURITY §3.
//!
//! "Images are off by default, with a per-document opt-in enabling local files
//! only. Remote schemes are never loaded — a placeholder renders instead, which
//! prevents a document phoning home and leaking that a particular file was
//! opened. Local paths resolve relative to the document's directory, rejecting
//! `..` traversal and absolute paths [...] 20 MB decode cap."
//!
//! # Why this is a command rather than the asset protocol
//!
//! SECURITY §3 names the Tauri asset protocol, "scoped at runtime to that
//! single directory and dropped when the document closes". The first half of
//! that is available and the second is not: Tauri's filesystem scope is
//! additive, and its `forbid_directory` is documented to take precedence over
//! allowed paths *always*. So the only way to revoke a grant is to add a
//! permanent denial — which would not drop the grant when the document closes,
//! it would poison that directory for every document opened from it afterwards.
//! A user who closed a book and reopened it would find its figures gone for the
//! rest of the session, with nothing to explain why.
//!
//! Going through a command instead makes the lifetime exact rather than
//! approximate. The grant *is* the entry in [`Documents`], which
//! `close_document` already removes, so a closed document cannot read anything
//! by construction — there is no scope to remember to revoke. It also keeps
//! this crate's existing rule intact: no `fs:` permission of any kind, and
//! every path the webview can reach is one a person chose in a native picker.
//! The webview never learns the document's directory; it sends the relative
//! path the file asked for, and this side decides what that means.
//!
//! The browser build has no command to call and so never loads a local image,
//! which is the same section's last sentence and needs no code to be true.

use std::path::{Component, Path, PathBuf};

use crate::document::Documents;
use crate::fs::{FileSystem, RealFs};

/// The decode cap from SECURITY §3.
///
/// Checked against the file's length before it is read, so an oversized image
/// is refused without being loaded into memory — a cap enforced after reading
/// is a cap on nothing.
const MAX_BYTES: u64 = 20 * 1024 * 1024;

/// Why a figure was not loaded, in words the interface can show.
///
/// Distinguished rather than collapsed into "no", because these mean different
/// things to the person reading: a missing file is something they can fix, a
/// rejected path is something the document did, and an oversized image is
/// neither. The placeholder says which.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The path is not a local relative path: a scheme, an absolute path, or
    /// traversal in any of its spellings.
    NotLocal,
    /// The document has never been saved, so there is no directory to be
    /// relative to.
    NoDirectory,
    /// The document is not open. A closed document reads nothing.
    Unknown,
    TooLarge {
        bytes: u64,
    },
    Missing,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotLocal => write!(formatter, "not a local file in the document's folder"),
            Self::NoDirectory => write!(formatter, "save the document first"),
            Self::Unknown => write!(formatter, "that document is not open"),
            Self::TooLarge { bytes } => {
                write!(formatter, "larger than 20 MB ({} bytes)", bytes)
            }
            Self::Missing => write!(formatter, "not found"),
        }
    }
}

/// Turns what a `\fig` asked for into a path inside `directory`, or refuses.
///
/// Decoded first, and repeatedly, because `%2e%2e%2f` is `../` and a check that
/// only sees the raw form is a check that has been walked around. Three rounds
/// rather than one: `%252e%252e%252f` decodes to `%2e%2e%2f` decodes to `../`.
///
/// The component walk is what actually rejects traversal. Comparing strings
/// against `".."` would miss a path that reaches the parent by some other
/// spelling, and would also reject a filename that merely contains two dots.
pub fn resolve(directory: &Path, asked: &str) -> Result<PathBuf, Refusal> {
    let decoded = decode(asked).ok_or(Refusal::NotLocal)?;
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        return Err(Refusal::NotLocal);
    }

    // Any scheme, not only the remote ones. `file:` is as much a way out of
    // the document's directory as `https:` is a way off the machine, and
    // `data:` has no `://` at all -- testing for that substring let it through
    // as an ordinary relative path.
    //
    // This is also what rejects `C:/Windows` everywhere rather than only on
    // Windows. `Component::Prefix` catches a drive letter on the platform that
    // has drive letters; on Unix the same string parses as an ordinary folder
    // called `C:`, so the check that mattered was passing for the wrong reason.
    if has_scheme(trimmed) {
        return Err(Refusal::NotLocal);
    }

    // Backslashes separate on Windows, so a check that only knows `/` is one
    // that `..\..\` walks straight through. Normalized before parsing, because
    // `Path` on Unix would read the whole thing as a single filename.
    let normalized = trimmed.replace('\\', "/");
    let candidate = Path::new(&normalized);

    let mut relative = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            // Harmless in itself and not worth carrying.
            Component::CurDir => {}
            // Everything that could leave the directory: `..`, a root, and a
            // Windows drive or share prefix.
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Refusal::NotLocal)
            }
        }
    }

    if relative.as_os_str().is_empty() {
        return Err(Refusal::NotLocal);
    }
    Ok(directory.join(relative))
}

/// Whether a path begins with a URL scheme.
///
/// The grammar rather than a list of the schemes worth worrying about: a list
/// is a thing to keep up to date, and the answer for a figure is the same for
/// every scheme there is.
fn has_scheme(path: &str) -> bool {
    let Some(colon) = path.find(':') else {
        return false;
    };
    let scheme = &path[..colon];

    !scheme.is_empty()
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Percent-decoding, until it stops changing.
///
/// `None` on malformed encoding, which is refused rather than passed through:
/// a path that cannot be checked cannot be cleared.
fn decode(raw: &str) -> Option<String> {
    let mut current = raw.to_string();

    for _ in 0..3 {
        let next = percent_decode(&current)?;
        if next == current {
            return Some(current);
        }
        current = next;
    }
    Some(current)
}

fn percent_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;

    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = raw.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    // Refused rather than replaced: bytes that are not text are not a path
    // anyone typed, and a lossy decode invents one.
    String::from_utf8(out).ok()
}

/// Reads a figure, having decided it may be read.
///
/// The containment check happens *after* canonicalizing, which is the point of
/// doing it twice. The component walk in [`resolve`] stops a path that spells
/// its way out; canonicalizing stops one that symlinks its way out, and only
/// the resolved path can be compared against the resolved directory.
pub fn read(
    filesystem: &impl FileSystem,
    directory: &Path,
    asked: &str,
) -> Result<Vec<u8>, Refusal> {
    let path = resolve(directory, asked)?;

    let real = filesystem
        .canonicalize(&path)
        .map_err(|_| Refusal::Missing)?;
    let root = filesystem
        .canonicalize(directory)
        .map_err(|_| Refusal::NoDirectory)?;
    if !real.starts_with(&root) {
        return Err(Refusal::NotLocal);
    }

    let meta = filesystem.metadata(&real).map_err(|_| Refusal::Missing)?;
    if meta.len > MAX_BYTES {
        return Err(Refusal::TooLarge { bytes: meta.len });
    }

    filesystem.read(&real).map_err(|_| Refusal::Missing)
}

/// The bytes of one figure, for a document the interface has open.
///
/// Bytes rather than a path or a URL. Handing back a path would mean the
/// webview held one, and the whole arrangement here is that it does not.
#[tauri::command]
pub fn read_figure(
    id: u64,
    path: String,
    documents: tauri::State<'_, Documents>,
) -> Result<Vec<u8>, String> {
    let directory = documents.directory_of(id).ok_or(Refusal::Unknown);

    let directory = match directory {
        Ok(Some(directory)) => directory,
        Ok(None) => return Err(Refusal::NoDirectory.to_string()),
        Err(refusal) => return Err(refusal.to_string()),
    };

    read(&RealFs, &directory, &path).map_err(|refusal| refusal.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/books/genesis")
    }

    #[test]
    fn resolves_a_plain_relative_path() {
        assert_eq!(
            resolve(&root(), "figures/map.png"),
            Ok(root().join("figures").join("map.png"))
        );
    }

    #[test]
    fn drops_a_leading_current_directory() {
        assert_eq!(resolve(&root(), "./map.png"), Ok(root().join("map.png")));
    }

    #[test]
    fn rejects_traversal() {
        assert_eq!(resolve(&root(), "../secrets.png"), Err(Refusal::NotLocal));
        assert_eq!(
            resolve(&root(), "figures/../../secrets.png"),
            Err(Refusal::NotLocal)
        );
    }

    #[test]
    fn rejects_traversal_spelled_with_backslashes() {
        // Windows separators, which a check that only knows about `/` reads as
        // one long filename and waves through.
        assert_eq!(resolve(&root(), "..\\secrets.png"), Err(Refusal::NotLocal));
        assert_eq!(
            resolve(&root(), "figures\\..\\..\\secrets.png"),
            Err(Refusal::NotLocal)
        );
    }

    #[test]
    fn rejects_percent_encoded_traversal() {
        assert_eq!(
            resolve(&root(), "%2e%2e%2fsecrets.png"),
            Err(Refusal::NotLocal)
        );
        assert_eq!(
            resolve(&root(), "%2E%2E/secrets.png"),
            Err(Refusal::NotLocal)
        );
        // Encoded backslash, which is both spellings at once.
        assert_eq!(
            resolve(&root(), "%2e%2e%5csecrets.png"),
            Err(Refusal::NotLocal)
        );
    }

    #[test]
    fn rejects_doubly_encoded_traversal() {
        // `%252e` decodes to `%2e` decodes to `.`, so one round of decoding
        // clears it and the path walks out anyway.
        assert_eq!(
            resolve(&root(), "%252e%252e%252fsecrets.png"),
            Err(Refusal::NotLocal)
        );
    }

    #[test]
    fn rejects_a_drive_letter_on_every_platform() {
        // Not through `Component::Prefix`, which only exists on Windows -- the
        // check has to hold on the platform where `C:/Windows` would otherwise
        // parse as a folder named `C:`.
        assert_eq!(
            resolve(&root(), "C:/Windows/win.ini"),
            Err(Refusal::NotLocal)
        );
        assert_eq!(resolve(&root(), "c:map.png"), Err(Refusal::NotLocal));
    }

    #[test]
    fn rejects_absolute_paths() {
        assert_eq!(resolve(&root(), "/etc/passwd"), Err(Refusal::NotLocal));
        assert_eq!(
            resolve(&root(), "C:/Windows/win.ini"),
            Err(Refusal::NotLocal)
        );
        assert_eq!(
            resolve(&root(), "\\\\server\\share\\x.png"),
            Err(Refusal::NotLocal)
        );
    }

    #[test]
    fn rejects_every_scheme() {
        // Remote ones leak that the file was opened; `file:` leaves the folder.
        for asked in [
            "https://evil.test/x.png",
            "http://evil.test/x.png",
            "file:///etc/passwd",
            "data:image/png;base64,AAAA",
        ] {
            assert_eq!(resolve(&root(), asked), Err(Refusal::NotLocal), "{asked}");
        }
    }

    #[test]
    fn rejects_a_scheme_hidden_by_encoding() {
        assert_eq!(
            resolve(&root(), "https%3A%2F%2Fevil.test%2Fx.png"),
            Err(Refusal::NotLocal)
        );
    }

    #[test]
    fn rejects_malformed_encoding() {
        // Cannot be checked, so it cannot be cleared.
        assert_eq!(resolve(&root(), "%zz.png"), Err(Refusal::NotLocal));
        assert_eq!(resolve(&root(), "map%2.png"), Err(Refusal::NotLocal));
    }

    #[test]
    fn rejects_nothing_at_all() {
        assert_eq!(resolve(&root(), ""), Err(Refusal::NotLocal));
        assert_eq!(resolve(&root(), "   "), Err(Refusal::NotLocal));
        assert_eq!(resolve(&root(), "./"), Err(Refusal::NotLocal));
    }

    #[test]
    fn keeps_a_filename_that_merely_contains_dots() {
        // The reason traversal is rejected by component rather than by
        // substring: this is an ordinary name.
        assert_eq!(
            resolve(&root(), "map..v2.png"),
            Ok(root().join("map..v2.png"))
        );
    }
}
