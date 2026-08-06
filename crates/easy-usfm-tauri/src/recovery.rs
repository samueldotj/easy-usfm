//! Recovery snapshots — FILE-FIDELITY §4, P4.1.
//!
//! "On 4 s idle after a change, and unconditionally every 45 s during
//! continuous typing. Written to `recovery/{blake3(canonical_path)[..16]}/` as
//! `snapshot.usfm` plus `meta.json` [...] using the rung-1 procedure — a torn
//! recovery file is worse than none. Last 3 retained; directories older than 30
//! days pruned at startup; cleared on clean save and clean close."
//!
//! The cadence is the interface's half — it is the side that knows when a
//! keystroke happened. This side owns where snapshots live, what they contain,
//! and how many survive.
//!
//! # Generations are directories
//!
//! §4 names two files, `snapshot.usfm` and `meta.json`, and asks for the last
//! three to be kept. Those cannot both be literal in one folder, so each
//! generation is its own directory named for the moment it was taken, holding
//! exactly the two files the section names.
//!
//! Rotating a fixed pair of names would have been the other reading, and it is
//! worse: renaming `snapshot.usfm` to `snapshot.1.usfm` before writing the new
//! one opens a window where the newest snapshot is the one that does not exist,
//! which is precisely the moment a crash is most likely — the application was
//! busy. A new directory each time means nothing that already survived is ever
//! touched to make room for something newer.
//!
//! # Why the path is hashed
//!
//! The directory name has to be stable across sessions, legal on every
//! filesystem, and bounded in length. A canonical path is none of those: it
//! contains separators, may contain characters Windows refuses, and can exceed
//! what a directory name allows. Hashing gives all three, and the original path
//! is kept in `meta.json` so the recovery prompt can say which file it means.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use easy_usfm_core::Eol;

use crate::fs::FileSystem;
use crate::save::{rung_one, RungOneFailure};

/// How many generations survive. FILE-FIDELITY §4.
const KEEP: usize = 3;

/// How long a document's recovery directory outlives its last snapshot.
const PRUNE_AFTER: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Everything a recovery prompt needs that the text does not carry.
///
/// The fidelity envelope is here because recovering a file means recovering
/// the bytes it would have been saved as, not merely its characters — a
/// recovery that silently changed a file's line endings would hand the user a
/// diff touching every line, which is the failure FILE-FIDELITY exists to
/// prevent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Meta {
    /// Where the document came from, or `None` for one never saved.
    pub path: Option<String>,
    pub bom: bool,
    /// The dominant terminator.
    ///
    /// The typed `Eol` rather than a string, so there is exactly one spelling
    /// of it. A free string held whichever the caller happened to send -- the
    /// status bar's `LF` from one path and the code's `lf` from another -- and
    /// P4.2 has to turn this back into a terminator to restore the envelope.
    /// Two spellings of the same fact, on the field that decides whether a
    /// recovered file keeps its line endings, is how a diff touching every line
    /// gets written.
    pub eol: Eol,
    pub final_newline: bool,
    /// Where the caret was, in UTF-16 code units.
    pub cursor: u32,
    pub dirty: bool,
    /// Which process wrote this, so a later session can tell a crash from a
    /// second window (P4.2 reads it; this only has to record it honestly).
    pub pid: u32,
    pub app_version: String,
    /// Milliseconds since the epoch, which is also the generation's name.
    pub taken_at: u64,
}

/// The directory for one document's snapshots.
///
/// Sixteen hex characters of BLAKE3 over the canonical path, per §4. Truncated
/// because this is a stable name, not a security boundary: two documents would
/// have to collide in 64 bits of hash *and* be open on the same machine for it
/// to matter, and the path inside `meta.json` says which one a snapshot is.
pub fn directory_for(root: &Path, canonical: &Path) -> PathBuf {
    let hash = blake3::hash(canonical.to_string_lossy().as_bytes());
    root.join(&hash.to_hex()[..16])
}

/// The name of a generation, which is when it was taken.
fn generation(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

/// Writes one snapshot, and drops whatever falls off the end.
///
/// The text and the metadata are written with the rung-1 procedure — a
/// temporary file and a rename — because a snapshot read back half-written is
/// worse than no snapshot at all. It would be offered to the user as their
/// recovered work.
///
/// Retention runs after the write rather than before it. Pruning first would
/// mean a crash between the two left the user with fewer snapshots than they
/// had a moment earlier, for the sake of a directory entry.
pub fn snapshot<F: FileSystem>(
    filesystem: &F,
    root: &Path,
    canonical: &Path,
    text: &str,
    meta: &Meta,
) -> std::io::Result<PathBuf> {
    let directory = directory_for(root, canonical);
    let generation = directory.join(meta.taken_at.to_string());
    filesystem.create_dir_all(&generation)?;

    let encoded = serde_json::to_vec_pretty(meta).map_err(std::io::Error::other)?;

    write_atomically(filesystem, &generation.join("snapshot.usfm"), text.as_bytes())?;
    write_atomically(filesystem, &generation.join("meta.json"), &encoded)?;

    retain(filesystem, &directory, KEEP);
    Ok(generation)
}

/// Rung 1, and nothing below it.
///
/// The save ladder steps down to non-atomic rungs for the *user's* file,
/// because refusing to save at all is worse than saving imperfectly. A
/// snapshot is the opposite trade: it is a safety net nobody asked for, and one
/// that can be read back torn is a net with a hole in it. If rung 1 will not
/// work here, the honest outcome is no snapshot.
fn write_atomically<F: FileSystem>(
    filesystem: &F,
    path: &Path,
    bytes: &[u8],
) -> std::io::Result<()> {
    rung_one(filesystem, path, bytes).map_err(|failure| match failure {
        RungOneFailure::Write(error)
        | RungOneFailure::Replace(error)
        | RungOneFailure::Sync(error) => error,
    })
}

/// Keeps the newest `keep` generations and removes the rest.
///
/// Sorted by the directory *name*, which is the timestamp it was taken at,
/// rather than by modification time. A filesystem that reports coarse or
/// wrong mtimes — network mounts and some containers do — would otherwise
/// delete the wrong three.
///
/// Failures are swallowed. Retention is housekeeping: a generation that could
/// not be removed costs disk, and reporting it would interrupt someone in the
/// middle of typing to tell them about a directory.
fn retain<F: FileSystem>(filesystem: &F, directory: &Path, keep: usize) {
    let Ok(entries) = filesystem.read_dir(directory) else {
        return;
    };

    let mut generations: Vec<(u64, PathBuf)> = entries
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            Some((name.parse::<u64>().ok()?, path))
        })
        .collect();

    // Newest first, so what is dropped is the tail.
    generations.sort_unstable_by_key(|(taken_at, _)| std::cmp::Reverse(*taken_at));

    for (_, path) in generations.into_iter().skip(keep) {
        let _ = filesystem.remove_dir_all(&path);
    }
}

/// Forgets a document's snapshots entirely.
///
/// FILE-FIDELITY §4: "cleared on clean save and clean close". Once the file on
/// disk holds the work, a snapshot of it is not a safety net — it is an offer
/// to restore something the user already has, which on the next launch reads as
/// the application having lost their save.
pub fn clear<F: FileSystem>(filesystem: &F, root: &Path, canonical: &Path) {
    let _ = filesystem.remove_dir_all(&directory_for(root, canonical));
}

/// Removes recovery directories nothing has written to in thirty days.
///
/// Run at startup, where a slow directory walk costs nothing anyone is waiting
/// on. Judged by the newest generation a directory holds, and a directory
/// holding no readable generation at all is removed too — that is either a
/// half-created one or something that is not ours.
pub fn prune<F: FileSystem>(filesystem: &F, root: &Path, now: SystemTime) {
    let Ok(documents) = filesystem.read_dir(root) else {
        return;
    };

    for document in documents {
        let newest = filesystem
            .read_dir(&document)
            .ok()
            .and_then(|generations| {
                generations
                    .iter()
                    .filter_map(|path| path.file_name()?.to_str()?.parse::<u64>().ok())
                    .max()
            });

        let stale = match newest {
            Some(taken) => older_than(taken, now, PRUNE_AFTER),
            None => true,
        };
        if stale {
            let _ = filesystem.remove_dir_all(&document);
        }
    }
}

/// Whether a generation is older than `age`.
///
/// A snapshot with a timestamp in the future is not stale. Clocks move
/// backwards — daylight saving on a filesystem timestamp, a corrected NTP
/// step, a dual-boot machine — and deleting someone's unsaved work because
/// their clock disagreed with ours would be the worst possible reading of it.
fn older_than(taken_at: u64, now: SystemTime, age: Duration) -> bool {
    let now = generation(now);
    now.saturating_sub(taken_at) > age.as_millis() as u64
}

// -------------------------------------------------------------- commands ---

/// Where snapshots live: `recovery/` inside the application's data directory.
///
/// Beside the user's file would be the other option and it is wrong twice: it
/// litters a translation folder that is very often a Git working tree, and it
/// fails outright on the read-only or network locations Scripture files are
/// frequently kept in.
fn root<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("recovery"))
        .map_err(|error| error.to_string())
}

/// The name a document is filed under.
///
/// Canonicalized where the file exists, so the same document reached by a
/// symlink, a mapped drive, or a different spelling of the same path finds its
/// own snapshots rather than starting a second set. A document never saved has
/// no path at all and is filed under a name derived from its session, because
/// the alternative is not snapshotting the case where losing work is most
/// likely -- there is no file on disk holding any of it.
fn key<F: FileSystem>(filesystem: &F, path: Option<&str>, id: u64) -> PathBuf {
    match path {
        Some(path) => {
            let path = Path::new(path);
            filesystem
                .canonicalize(path)
                .unwrap_or_else(|_| path.to_path_buf())
        }
        None => PathBuf::from(format!("unsaved-{id}")),
    }
}

/// Takes one snapshot. Called by the interface on its own cadence.
#[tauri::command]
pub fn snapshot_document<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: u64,
    path: Option<String>,
    text: String,
    mut meta: Meta,
) -> Result<(), String> {
    let root = root(&app)?;
    let filesystem = crate::fs::RealFs;

    // Recorded here rather than taken from the interface: a snapshot claiming
    // to be from a process that never wrote it would make P4.2 read a crash
    // where there was none.
    meta.pid = std::process::id();
    meta.app_version = env!("CARGO_PKG_VERSION").to_string();
    meta.taken_at = generation(SystemTime::now());
    meta.path = path.clone();

    snapshot(
        &filesystem,
        &root,
        &key(&filesystem, path.as_deref(), id),
        &text,
        &meta,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// Forgets a document's snapshots, on a clean save or a clean close.
#[tauri::command]
pub fn clear_recovery<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: u64,
    path: Option<String>,
) -> Result<(), String> {
    let root = root(&app)?;
    let filesystem = crate::fs::RealFs;
    clear(&filesystem, &root, &key(&filesystem, path.as_deref(), id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::RealFs;

    fn meta(at: u64) -> Meta {
        Meta {
            path: Some("/books/GEN.usfm".to_string()),
            bom: false,
            eol: Eol::Lf,
            final_newline: true,
            cursor: 0,
            dirty: true,
            pid: 1,
            app_version: "0.1.0".to_string(),
            taken_at: at,
        }
    }

    #[test]
    fn a_saved_document_is_filed_under_its_real_path() {
        // Canonicalized, so the same file reached by a different spelling --
        // a symlink, a mapped drive, `./` in the middle -- finds the snapshots
        // it already has rather than starting a second set beside them.
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("GEN.usfm");
        std::fs::write(&file, b"anything").unwrap();

        let direct = key(&RealFs, file.to_str(), 1);
        let roundabout = key(&RealFs, temp.path().join(".").join("GEN.usfm").to_str(), 1);

        assert_eq!(direct, roundabout);
    }

    #[test]
    fn a_path_that_cannot_be_resolved_is_still_filed() {
        // A file on a disconnected network share, or one deleted while open.
        // Falling back to the path as written keeps snapshotting rather than
        // silently stopping for the document most at risk.
        let asked = "/nowhere/GEN.usfm";
        assert_eq!(key(&RealFs, Some(asked), 1), PathBuf::from(asked));
    }

    #[test]
    fn an_unsaved_document_is_filed_under_its_session() {
        // Nothing on disk holds any of this work, which makes it the case
        // where a snapshot matters most -- so it gets one rather than being
        // skipped for want of a name.
        assert_eq!(key(&RealFs, None, 7), PathBuf::from("unsaved-7"));
        assert_ne!(key(&RealFs, None, 7), key(&RealFs, None, 8));
    }

    #[test]
    fn the_directory_is_stable_and_short() {
        let root = Path::new("/app/recovery");
        let first = directory_for(root, Path::new("/books/GEN.usfm"));

        assert_eq!(first, directory_for(root, Path::new("/books/GEN.usfm")));
        assert_ne!(first, directory_for(root, Path::new("/books/EXO.usfm")));

        let name = first.file_name().unwrap().to_str().unwrap();
        assert_eq!(name.len(), 16);
        assert!(name.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn a_snapshot_holds_the_text_and_the_envelope() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let document = Path::new("/books/GEN.usfm");

        let written = snapshot(&RealFs, root, document, "\\id GEN\n", &meta(1000)).unwrap();

        assert_eq!(
            std::fs::read_to_string(written.join("snapshot.usfm")).unwrap(),
            "\\id GEN\n"
        );
        let stored: Meta =
            serde_json::from_slice(&std::fs::read(written.join("meta.json")).unwrap()).unwrap();
        assert_eq!(stored, meta(1000));
    }

    #[test]
    fn only_the_newest_three_survive() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let document = Path::new("/books/GEN.usfm");

        for at in [100u64, 200, 300, 400, 500] {
            snapshot(&RealFs, root, document, &format!("text {at}"), &meta(at)).unwrap();
        }

        let kept: Vec<u64> = std::fs::read_dir(directory_for(root, document))
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
            })
            .collect();

        let mut sorted = kept.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, vec![300, 400, 500], "kept {kept:?}");
    }

    #[test]
    fn retention_goes_by_the_name_not_the_clock() {
        // Every generation is written within the same millisecond of wall
        // clock here, so anything sorting by mtime would keep an arbitrary
        // three. The name is the timestamp and is the only ordering that holds.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let document = Path::new("/books/GEN.usfm");

        for at in [9u64, 10, 11, 12] {
            snapshot(&RealFs, root, document, "x", &meta(at)).unwrap();
        }

        assert!(directory_for(root, document).join("12").exists());
        assert!(!directory_for(root, document).join("9").exists());
    }

    #[test]
    fn clearing_forgets_the_document() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let document = Path::new("/books/GEN.usfm");

        snapshot(&RealFs, root, document, "x", &meta(1)).unwrap();
        assert!(directory_for(root, document).exists());

        clear(&RealFs, root, document);
        assert!(!directory_for(root, document).exists());
    }

    #[test]
    fn pruning_removes_only_what_nobody_is_coming_back_for() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(60 * 24 * 60 * 60);
        let now_ms = generation(now);

        let recent = Path::new("/books/RECENT.usfm");
        let ancient = Path::new("/books/ANCIENT.usfm");
        snapshot(&RealFs, root, recent, "x", &meta(now_ms - 1000)).unwrap();
        snapshot(
            &RealFs,
            root,
            ancient,
            "x",
            &meta(now_ms - 31 * 24 * 60 * 60 * 1000),
        )
        .unwrap();

        prune(&RealFs, root, now);

        assert!(directory_for(root, recent).exists());
        assert!(!directory_for(root, ancient).exists());
    }

    #[test]
    fn pruning_keeps_a_snapshot_from_the_future() {
        // A clock that moved backwards -- daylight saving, an NTP step, a
        // dual-boot machine. Deleting unsaved work over a disagreement about
        // the time would be the worst available reading.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(60 * 24 * 60 * 60);
        let document = Path::new("/books/GEN.usfm");

        snapshot(&RealFs, root, document, "x", &meta(generation(now) + 60_000)).unwrap();
        prune(&RealFs, root, now);

        assert!(directory_for(root, document).exists());
    }

    #[test]
    fn pruning_removes_a_directory_with_nothing_readable_in_it() {
        let temp = tempfile::tempdir().unwrap();
        let stray = temp.path().join("not-a-generation");
        std::fs::create_dir_all(stray.join("whatever")).unwrap();

        prune(&RealFs, temp.path(), SystemTime::now());

        assert!(!stray.exists());
    }
}
