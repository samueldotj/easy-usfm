//! The atomic save ladder.
//!
//! FILE-FIDELITY §2, and one rule above all the others:
//!
//! > **The original is never truncated before the replacement is durable.**
//!
//! Three rungs, chosen per platform and per filesystem. Rung 1 replaces the
//! file by renaming a fully-written temp over it. Rung 2 keeps the inode and
//! writes in place behind a sidecar, for cloud-sync roots and hardlinks where
//! replacing the inode causes real damage. Rung 3 does not write at all.
//!
//! And one rule about failure:
//!
//! > **No failure path leaves the editor believing it saved.**
//!
//! Every error returned from here names the file that still holds the previous
//! content, and gives the sidecar path where one exists.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::fs::FileSystem;

/// Which rung was used, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rung {
    /// Durable rename. The default, and the only atomic one.
    Rename,
    /// Copy-back, preserving the inode.
    CopyBack,
}

/// Why the ladder stepped down to rung 2.
///
/// Surfaced in the status bar, because a slower save with no visible reason
/// reads as the application being slow (FILE-FIDELITY §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CopyBackReason {
    /// `st_nlink > 1`. Rung 1 would break the link silently and the other
    /// names would keep pointing at the old content.
    HardLinked,
    /// The target sits under a known cloud-sync root. Sync clients hold open
    /// handles and watch inodes; replacing the inode reads to them as
    /// delete-plus-create, which produces conflicted copies and on some
    /// clients loses data.
    SyncRoot,
    /// Rung 1 was tried and refused.
    RenameFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub path: PathBuf,
    pub rung: Rung,
    pub reason: Option<CopyBackReason>,
}

/// A save that did not happen.
#[derive(Debug)]
pub enum SaveError {
    /// The file cannot be written to. No write was attempted; the caller
    /// offers Save As instead of reporting a failure the user cannot act on.
    ReadOnly { path: PathBuf },
    /// Everything else. `intact` names the file that still holds the previous
    /// content, and `sidecar` the copy left behind if one was made.
    Failed {
        intact: PathBuf,
        sidecar: Option<PathBuf>,
        source: io::Error,
    },
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly { path } => {
                write!(f, "{} is read-only; nothing was written", path.display())
            }
            Self::Failed {
                intact,
                sidecar,
                source,
            } => {
                write!(
                    f,
                    "{source}. {} still holds the previous content",
                    intact.display()
                )?;
                if let Some(sidecar) = sidecar {
                    write!(f, "; a copy was left at {}", sidecar.display())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for SaveError {}

/// Writes `bytes` to `path`, atomically where the filesystem allows it.
pub fn save<F: FileSystem>(fs: &F, path: &Path, bytes: &[u8]) -> Result<Saved, SaveError> {
    // Resolve the link, not the name. Renaming over a symlink replaces the
    // link itself and silently detaches the file the user opened.
    let target = fs.canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    let existing = fs.exists(&target);
    let meta = if existing {
        fs.metadata(&target).ok()
    } else {
        None
    };

    // ---- rung 3 ----
    //
    // No write attempted. There is nothing to recover from and nothing to
    // report as a failure: the answer is Save As.
    if meta.as_ref().is_some_and(|meta| meta.readonly) {
        return Err(SaveError::ReadOnly {
            path: target.clone(),
        });
    }

    let forced = if meta.as_ref().is_some_and(|meta| meta.links > 1) {
        Some(CopyBackReason::HardLinked)
    } else if under_sync_root(&target) {
        Some(CopyBackReason::SyncRoot)
    } else {
        None
    };

    if let Some(reason) = forced {
        return copy_back(fs, &target, bytes, reason);
    }

    match rung_one(fs, &target, bytes) {
        Ok(()) => Ok(Saved {
            path: target,
            rung: Rung::Rename,
            reason: None,
        }),

        // Only a refused *replace* steps down a rung. Sharing violations and
        // cross-device renames land here, and both are exactly what rung 2
        // exists for.
        Err(RungOneFailure::Replace(source)) if existing => {
            match copy_back(fs, &target, bytes, CopyBackReason::RenameFailed) {
                Ok(saved) => Ok(saved),
                // Rung 2 failed too. Report the reason it was reached, not the
                // second failure, or the user is told about a copy-back they
                // never asked for instead of the sharing violation behind it.
                Err(SaveError::Failed {
                    intact, sidecar, ..
                }) => Err(SaveError::Failed {
                    intact,
                    sidecar,
                    source,
                }),
                Err(other) => Err(other),
            }
        }

        // A failed *write* does not step down. Rung 2 truncates the original
        // before writing, so retrying a write that just failed -- for no space,
        // or a vanished directory -- would destroy the file it is trying to
        // save. The sidecar would survive it, but a save path whose recovery
        // story is "the backup still exists" is not a save path.
        Err(failure) => Err(SaveError::Failed {
            intact: target,
            sidecar: None,
            source: failure.into_error(),
        }),
    }
}

/// Which step of rung 1 gave way. The distinction decides whether stepping
/// down to rung 2 is safe.
enum RungOneFailure {
    Write(io::Error),
    Replace(io::Error),
    Sync(io::Error),
}

impl RungOneFailure {
    fn into_error(self) -> io::Error {
        match self {
            Self::Write(error) | Self::Replace(error) | Self::Sync(error) => error,
        }
    }
}

/// Rung 1 — write a temp beside the target, then replace.
fn rung_one<F: FileSystem>(fs: &F, target: &Path, bytes: &[u8]) -> Result<(), RungOneFailure> {
    let temp = temp_beside(target);

    // The temp is created in the target's own directory, never in $TMPDIR: a
    // temp on another filesystem makes `rename` non-atomic and often makes it
    // fail outright.
    fs.write_new(&temp, bytes).map_err(RungOneFailure::Write)?;

    // Carry the original's ownership and mode onto the replacement before it
    // becomes the file.
    if fs.exists(target) {
        let _ = fs.copy_attributes(target, &temp);
    }

    if let Err(error) = fs.rename(&temp, target) {
        // The original is untouched; clear the temp away rather than leaving
        // litter beside the user's file.
        let _ = fs.remove(&temp);
        return Err(RungOneFailure::Replace(error));
    }

    if let Some(directory) = target.parent() {
        fs.sync_dir(directory).map_err(RungOneFailure::Sync)?;
    }
    Ok(())
}

/// Rung 2 — keep the inode, write in place, behind a sidecar.
///
/// Genuinely non-atomic, which is why it is the second rung. The sidecar is
/// the answer to "what if we crash inside the in-place write".
fn copy_back<F: FileSystem>(
    fs: &F,
    target: &Path,
    bytes: &[u8],
    reason: CopyBackReason,
) -> Result<Saved, SaveError> {
    let failed = |source: io::Error, sidecar: Option<PathBuf>| SaveError::Failed {
        intact: target.to_path_buf(),
        sidecar,
        source,
    };

    if !fs.exists(target) {
        // Nothing to preserve the inode of. A plain durable write is both
        // sufficient and atomic enough.
        fs.write_new(target, bytes).map_err(|e| failed(e, None))?;
        return Ok(Saved {
            path: target.to_path_buf(),
            rung: Rung::CopyBack,
            reason: Some(reason),
        });
    }

    let sidecar = sidecar_beside(target);
    fs.copy(target, &sidecar).map_err(|e| failed(e, None))?;

    match fs.write_in_place(target, bytes) {
        Ok(()) => {
            let _ = fs.remove(&sidecar);
            Ok(Saved {
                path: target.to_path_buf(),
                rung: Rung::CopyBack,
                reason: Some(reason),
            })
        }
        // The in-place write is where content can be lost, so the sidecar is
        // kept and its path surfaced rather than cleaned up.
        Err(source) => Err(failed(source, Some(sidecar))),
    }
}

fn temp_beside(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".to_string());

    let unique = format!(
        ".{name}.{}.{:x}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    );

    target.with_file_name(unique)
}

fn sidecar_beside(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "document".to_string());

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    target.with_file_name(format!("{name}.bak-{stamp}"))
}

/// Whether the path sits under a known cloud-sync root.
///
/// Matched on the path rather than probed, because the damage happens on the
/// first save and a probe would have to cause it to detect it.
fn under_sync_root(path: &Path) -> bool {
    const MARKERS: &[&str] = &[
        "dropbox",
        "onedrive",
        "google drive",
        "googledrive",
        "cloudstorage", // ~/Library/CloudStorage, macOS
        "icloud drive",
        "com~apple~clouddocs",
        "nextcloud",
        "syncthing",
    ];

    // Both separators, always. Windows accepts forward slashes throughout, so
    // splitting on MAIN_SEPARATOR alone means a perfectly ordinary
    // `C:/Users/x/Dropbox/gen.usfm` is never recognised and every save into it
    // silently takes the inode-replacing rung -- which is the conflicted-copy
    // damage this check exists to avoid.
    let lowered = path.to_string_lossy().to_lowercase();
    MARKERS.iter().any(|marker| {
        lowered
            .split(['/', '\\'])
            .any(|segment| segment == *marker || segment.starts_with(&format!("{marker} ")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_roots_are_recognised_by_directory_name() {
        assert!(under_sync_root(Path::new("/home/x/Dropbox/gen.usfm")));
        assert!(under_sync_root(Path::new(
            "/Users/x/Library/CloudStorage/a.usfm"
        )));
        assert!(under_sync_root(Path::new("/home/x/OneDrive/gen.usfm")));
        // A personal OneDrive is named "OneDrive - Contoso".
        assert!(under_sync_root(Path::new(
            "/home/x/OneDrive - Acme/gen.usfm"
        )));
    }

    #[test]
    fn an_ordinary_path_is_not_a_sync_root() {
        assert!(!under_sync_root(Path::new("/home/x/translations/gen.usfm")));
        // Substring matches must not count, or every file in a directory whose
        // name merely contains "dropbox" would take the slow path forever.
        assert!(!under_sync_root(Path::new(
            "/home/x/my-dropbox-notes/gen.usfm"
        )));
    }

    #[test]
    fn the_temp_file_is_beside_its_target() {
        // Not in $TMPDIR: a temp on another filesystem makes rename
        // non-atomic and often makes it fail outright.
        let target = Path::new("/home/x/translations/gen.usfm");
        let temp = temp_beside(target);

        assert_eq!(temp.parent(), target.parent());
        assert!(temp
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".gen.usfm."));
    }

    #[test]
    fn the_sidecar_is_beside_its_target() {
        let target = Path::new("/home/x/translations/gen.usfm");
        let sidecar = sidecar_beside(target);

        assert_eq!(sidecar.parent(), target.parent());
        assert!(sidecar
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gen.usfm.bak-"));
    }
}
