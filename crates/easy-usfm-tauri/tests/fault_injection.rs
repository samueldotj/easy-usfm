//! The save ladder under failure — P1.8, FILE-FIDELITY §5.2.
//!
//! Every case here asserts the same two things, which are the whole point:
//!
//! > **Each must leave the original intact and the document dirty.**
//!
//! "Document dirty" is the caller's business, and is expressed here as the
//! save returning `Err` — a save that returns `Ok` is one the editor will
//! believe, so anything that can go wrong must not return `Ok`.
//!
//! These run against a real temporary directory with one operation sabotaged,
//! rather than against a fake filesystem. A fully faked filesystem tests the
//! fake; a real one with a single injected fault tests the ladder.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use easy_usfm_tauri_lib::fs::{FileMeta, FileSystem, RealFs};
use easy_usfm_tauri_lib::save::{save, CopyBackReason, Rung, SaveError};

/// Which step to break.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    None,
    /// ENOSPC mid-write: the temp file cannot be written.
    WriteNew,
    /// EACCES on the rename.
    Rename,
    /// The in-place write of rung 2 fails after the sidecar exists.
    WriteInPlace,
    /// The parent directory vanished between the read and the write.
    ParentGone,
    /// The sidecar copy fails.
    Copy,
}

/// A real filesystem with one operation sabotaged.
struct FaultyFs {
    inner: RealFs,
    fault: Fault,
    /// Every path the ladder wrote to, so a test can assert nothing was
    /// touched that should not have been.
    touched: RefCell<Vec<PathBuf>>,
    /// Reports this many links, to force the hardlink path.
    links: u64,
}

impl FaultyFs {
    fn new(fault: Fault) -> Self {
        Self {
            inner: RealFs,
            fault,
            touched: RefCell::new(Vec::new()),
            links: 1,
        }
    }

    fn with_links(links: u64) -> Self {
        Self {
            links,
            ..Self::new(Fault::None)
        }
    }

    fn fails(&self, fault: Fault) -> bool {
        self.fault == fault
    }
}

fn denied(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, format!("injected: {what}"))
}

impl FileSystem for FaultyFs {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.inner.read(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMeta> {
        let mut meta = self.inner.metadata(path)?;
        meta.links = self.links;
        Ok(meta)
    }

    fn exists(&self, path: &Path) -> bool {
        self.inner.exists(path)
    }

    // Directory operations pass straight through. The faults this fake injects
    // are the save ladder's -- a refused write, a refused replace, a failed
    // sync -- and none of them is about a directory listing.
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.create_dir_all(path)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        self.inner.read_dir(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.inner.remove_dir_all(path)
    }

    fn modified(&self, path: &Path) -> io::Result<std::time::SystemTime> {
        self.inner.modified(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.inner.canonicalize(path)
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.fails(Fault::WriteNew) {
            return Err(io::Error::new(
                io::ErrorKind::StorageFull,
                "injected: ENOSPC",
            ));
        }
        if self.fails(Fault::ParentGone) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "injected: no parent",
            ));
        }
        self.touched.borrow_mut().push(path.to_path_buf());
        self.inner.write_new(path, bytes)
    }

    fn write_in_place(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        if self.fails(Fault::WriteInPlace) {
            return Err(denied("in-place write"));
        }
        self.touched.borrow_mut().push(path.to_path_buf());
        self.inner.write_in_place(path, bytes)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        if self.fails(Fault::Rename) {
            return Err(denied("rename"));
        }
        self.inner.rename(from, to)
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<()> {
        if self.fails(Fault::Copy) {
            return Err(denied("copy"));
        }
        self.inner.copy(from, to)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        self.inner.remove(path)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        self.inner.sync_dir(dir)
    }

    fn copy_attributes(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.inner.copy_attributes(from, to)
    }
}

// ------------------------------------------------------------- fixtures ---

const ORIGINAL: &[u8] = b"\\id GEN Genesis\r\n\\c 1\r\n\\v 1 the original\r\n";
const REPLACEMENT: &[u8] = b"\\id GEN Genesis\r\n\\c 1\r\n\\v 1 the replacement\r\n";

struct Fixture {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("gen.usfm");
    std::fs::write(&path, ORIGINAL).expect("write fixture");
    Fixture { _dir: dir, path }
}

fn contents(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("read")
}

/// Nothing beside the target survived the attempt.
fn no_litter(dir: &Path, target: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .expect("read dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path != target)
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect()
}

// ---------------------------------------------------------- happy paths ---

#[test]
fn a_successful_save_replaces_the_contents_and_leaves_nothing_behind() {
    let file = fixture();
    let fs = FaultyFs::new(Fault::None);

    let saved = save(&fs, &file.path, REPLACEMENT).expect("save succeeds");

    assert_eq!(saved.rung, Rung::Rename);
    assert_eq!(contents(&file.path), REPLACEMENT);
    assert_eq!(
        no_litter(file.path.parent().unwrap(), &file.path),
        Vec::<String>::new(),
        "a temp or sidecar was left beside the file"
    );
}

#[test]
fn a_hardlinked_file_takes_the_copy_back_rung() {
    // Rung 1 would break the link silently and the other names would keep
    // pointing at the old content.
    let file = fixture();
    let fs = FaultyFs::with_links(2);

    let saved = save(&fs, &file.path, REPLACEMENT).expect("save succeeds");

    assert_eq!(saved.rung, Rung::CopyBack);
    assert_eq!(saved.reason, Some(CopyBackReason::HardLinked));
    assert_eq!(contents(&file.path), REPLACEMENT);
}

// --------------------------------------------------------- the six cases ---

#[test]
fn enospc_mid_write_leaves_the_original_intact() {
    let file = fixture();
    let fs = FaultyFs::new(Fault::WriteNew);

    let error = save(&fs, &file.path, REPLACEMENT).expect_err("must not report success");

    assert!(matches!(error, SaveError::Failed { .. }));
    assert_eq!(contents(&file.path), ORIGINAL, "the original was damaged");
}

#[test]
fn a_refused_rename_falls_back_to_copy_back_and_still_saves() {
    // Sharing violations and cross-device renames both land here, and both are
    // exactly what rung 2 exists for.
    let file = fixture();
    let fs = FaultyFs::new(Fault::Rename);

    let saved = save(&fs, &file.path, REPLACEMENT).expect("rung 2 takes over");

    assert_eq!(saved.rung, Rung::CopyBack);
    assert_eq!(saved.reason, Some(CopyBackReason::RenameFailed));
    assert_eq!(contents(&file.path), REPLACEMENT);
    assert_eq!(
        no_litter(file.path.parent().unwrap(), &file.path),
        Vec::<String>::new(),
        "the failed rung 1 left its temp behind"
    );
}

#[test]
fn a_vanished_parent_directory_leaves_the_original_intact() {
    let file = fixture();
    let fs = FaultyFs::new(Fault::ParentGone);

    let error = save(&fs, &file.path, REPLACEMENT).expect_err("must not report success");

    assert!(matches!(error, SaveError::Failed { .. }));
    assert_eq!(contents(&file.path), ORIGINAL);
}

#[test]
fn a_failed_in_place_write_keeps_the_sidecar_and_names_it() {
    // The one place content can actually be lost, which is why the sidecar
    // exists and why it is not cleaned up on failure.
    let file = fixture();
    let fs = FaultyFs {
        fault: Fault::WriteInPlace,
        ..FaultyFs::with_links(2)
    };

    let error = save(&fs, &file.path, REPLACEMENT).expect_err("must not report success");

    let SaveError::Failed {
        sidecar, intact, ..
    } = error
    else {
        panic!("expected a failure carrying a sidecar");
    };

    let sidecar = sidecar.expect("the sidecar path must be surfaced");
    assert!(sidecar.exists(), "the sidecar was cleaned up on failure");
    assert_eq!(contents(&sidecar), ORIGINAL, "the sidecar lost the content");
    // The reported path is the canonical one -- the ladder replaces the target
    // of a link, never the link.
    assert_eq!(intact, std::fs::canonicalize(&file.path).unwrap());
}

#[test]
fn a_failed_write_does_not_fall_back_to_truncating_the_original() {
    // Found by this suite on its first run. Falling back to rung 2 after a
    // *write* failure means truncating the file and then attempting the write
    // that just failed -- on a full disk, destroying the document being saved.
    // Only a refused replace steps down a rung.
    let file = fixture();
    let fs = FaultyFs::new(Fault::WriteNew);

    let error = save(&fs, &file.path, REPLACEMENT).expect_err("must not report success");

    assert!(matches!(error, SaveError::Failed { sidecar: None, .. }));
    assert_eq!(contents(&file.path), ORIGINAL);
    assert!(
        fs.touched.borrow().is_empty(),
        "the original was opened for writing after the write had already failed"
    );
}

#[test]
fn a_failed_sidecar_copy_never_touches_the_original() {
    // If the copy fails there is no safety net, so the in-place write must not
    // be attempted at all.
    let file = fixture();
    let fs = FaultyFs {
        fault: Fault::Copy,
        ..FaultyFs::with_links(2)
    };

    let error = save(&fs, &file.path, REPLACEMENT).expect_err("must not report success");

    assert!(matches!(error, SaveError::Failed { .. }));
    assert_eq!(contents(&file.path), ORIGINAL);
    assert!(
        fs.touched.borrow().is_empty(),
        "the original was opened for writing with no sidecar in place"
    );
}

#[test]
fn a_read_only_file_is_refused_before_anything_is_written() {
    let file = fixture();
    let mut permissions = std::fs::metadata(&file.path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&file.path, permissions).unwrap();

    let fs = FaultyFs::new(Fault::None);
    let error = save(&fs, &file.path, REPLACEMENT).expect_err("must not report success");

    // Rung 3: the answer is Save As, not a failure the user cannot act on.
    assert!(matches!(error, SaveError::ReadOnly { .. }), "{error}");
    assert_eq!(contents(&file.path), ORIGINAL);

    // Restored so the temporary directory can be removed. clippy warns that
    // clearing the read-only bit grants everyone write access on Unix, which
    // is true and is the point: this is undoing the test's own setup on a
    // file that is about to be deleted.
    #[allow(clippy::permissions_set_readonly_false)]
    {
        let mut permissions = std::fs::metadata(&file.path).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&file.path, permissions).unwrap();
    }
}

// -------------------------------------------------------------- fidelity ---

#[test]
fn saving_preserves_the_bytes_it_was_given_exactly() {
    // The ladder writes bytes and forms no opinion about them. Whatever the
    // fidelity envelope produced is what reaches the disk.
    let file = fixture();
    let fs = FaultyFs::new(Fault::None);

    let awkward = "\u{feff}\\id GEN\r\n\\v 1 க்ஷேமம்\r\\v 2 שלום\n".as_bytes();
    save(&fs, &file.path, awkward).expect("save succeeds");

    assert_eq!(contents(&file.path), awkward);
}
