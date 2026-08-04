//! The filesystem, behind a trait.
//!
//! FILE-FIDELITY §5.2: file I/O goes through a trait "so §2's failure paths
//! are testable rather than aspirational". The interesting cases in a save
//! ladder are all failures — a full disk mid-write, a permission error on the
//! rename, a directory that vanished — and none of them can be produced on
//! demand against a real filesystem.
//!
//! The trait is deliberately *lower level* than the `write_atomic` sketched in
//! that section. A trait at that level could only be faked wholesale, which
//! would test the fake rather than the ladder; the six required fault cases
//! all name a specific step, so the steps are what the trait exposes.

use std::io;
use std::path::{Path, PathBuf};

/// What the save ladder needs to know about a file it is about to replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    pub readonly: bool,
    /// Number of names pointing at this file's inode.
    ///
    /// Above one, rung 1 breaks the link silently and the other names keep
    /// pointing at the old content (FILE-FIDELITY §2). Always 1 on platforms
    /// that do not report it.
    pub links: u64,
    pub len: u64,
}

/// Every filesystem operation the ladder performs.
pub trait FileSystem {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn metadata(&self, path: &Path) -> io::Result<FileMeta>;
    fn exists(&self, path: &Path) -> bool;

    /// Resolves symlinks. The ladder replaces the *target*, never the link —
    /// renaming over a symlink would replace the link itself and silently
    /// detach the file the user opened.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;

    /// Creates a file and writes it durably. Fails if it already exists.
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    /// Truncates an existing file and writes it durably, preserving the inode.
    /// Rung 2's step 3.
    fn write_in_place(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn copy(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()>;

    /// Makes a rename durable by syncing the directory that contains it.
    ///
    /// Commonly omitted, and it matters: without it a power loss can leave the
    /// directory entry unpersisted — temp gone, rename lost, original
    /// unreferenced (FILE-FIDELITY §2).
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;

    /// Copies ownership, mode, ACLs, and extended attributes onto the
    /// replacement, so the file the user ends up with is the file they had.
    fn copy_attributes(&self, from: &Path, to: &Path) -> io::Result<()>;
}

/// The real one.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFs;

impl FileSystem for RealFs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMeta> {
        let meta = std::fs::metadata(path)?;
        Ok(FileMeta {
            readonly: meta.permissions().readonly(),
            links: link_count(&meta),
            len: meta.len(),
        })
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::canonicalize(path)
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        // Durable before it is renamed into place, or the rename can publish a
        // file whose contents never reached the disk.
        file.sync_all()
    }

    fn write_in_place(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        platform::replace(from, to)
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::copy(from, to).map(|_| ())
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        platform::sync_dir(dir)
    }

    fn copy_attributes(&self, from: &Path, to: &Path) -> io::Result<()> {
        platform::copy_attributes(from, to)
    }
}

#[cfg(unix)]
fn link_count(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.nlink()
}

#[cfg(not(unix))]
fn link_count(_meta: &std::fs::Metadata) -> u64 {
    // Windows has hardlinks but reports the count only through
    // GetFileInformationByHandle. Reporting 1 means rung 1 is attempted and
    // the link is broken silently, which is a real gap -- recorded here rather
    // than hidden, and closed when P1.7's detection is verified on Windows.
    1
}

// ------------------------------------------------------------- platform ---

#[cfg(unix)]
mod platform {
    use std::io;
    use std::path::Path;

    /// `rename` is atomic on the same filesystem, which is why the temp file
    /// is created beside the target rather than in `$TMPDIR`.
    pub fn replace(from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    pub fn sync_dir(dir: &Path) -> io::Result<()> {
        // Opening a directory read-only and fsyncing it is how the rename
        // itself is made durable.
        let handle = std::fs::File::open(dir)?;
        handle.sync_all()
    }

    pub fn copy_attributes(from: &Path, to: &Path) -> io::Result<()> {
        // Mode is what the standard library can carry. Extended attributes and
        // ACLs need copyfile(3) on macOS and listxattr/getxattr/setxattr on
        // Linux, which is P1.5's remaining work and needs a POSIX machine to
        // verify -- it is not written blind here.
        let permissions = std::fs::metadata(from)?.permissions();
        std::fs::set_permissions(to, permissions)
    }
}

#[cfg(windows)]
mod platform {
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Storage::FileSystem::{ReplaceFileW, REPLACEFILE_WRITE_THROUGH};

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    /// `ReplaceFileW`, not `MoveFileEx`.
    ///
    /// FILE-FIDELITY §2: `ReplaceFileW` preserves the destination's ACLs,
    /// attributes, creation time, object ID, and alternate data streams.
    /// `MoveFileEx` discards all of them, which on Windows means a save
    /// quietly strips the permissions and metadata a file arrived with.
    pub fn replace(from: &Path, to: &Path) -> io::Result<()> {
        // A destination that does not exist yet is a plain rename; ReplaceFileW
        // requires something to replace.
        if !to.exists() {
            return std::fs::rename(from, to);
        }

        let replaced = wide(to);
        let replacement = wide(from);

        // SAFETY: both strings are NUL-terminated and live for the call.
        let ok = unsafe {
            ReplaceFileW(
                replaced.as_ptr(),
                replacement.as_ptr(),
                std::ptr::null(),
                REPLACEFILE_WRITE_THROUGH,
                std::ptr::null(),
                std::ptr::null(),
            )
        };

        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn sync_dir(_dir: &Path) -> io::Result<()> {
        // REPLACEFILE_WRITE_THROUGH already commits the metadata change, and
        // Windows does not let a directory be opened for fsync the way POSIX
        // does. Nothing to do rather than nothing forgotten.
        Ok(())
    }

    pub fn copy_attributes(_from: &Path, _to: &Path) -> io::Result<()> {
        // ReplaceFileW carries them across as part of the replacement, which
        // is the whole reason for preferring it.
        Ok(())
    }
}
