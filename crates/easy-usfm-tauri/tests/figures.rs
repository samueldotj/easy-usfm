//! The figure policy against a real filesystem — SECURITY §3.
//!
//! The unit tests in `figure.rs` reason about paths as strings, which is where
//! the traversal spellings live. What they cannot reach is the half of the
//! check that only exists once there are real files: canonicalization. A path
//! made only of ordinary components can still leave the folder if one of them
//! is a symlink, and no amount of string inspection sees that.
//!
//! So these tests build a document folder with a file next to it that must stay
//! unreachable, and try to reach it.

use std::fs;
use std::path::Path;

use easy_usfm_tauri_lib::figure::{read, Refusal};
use easy_usfm_tauri_lib::fs::RealFs;

/// A folder to be a document's, and a secret beside it that is not.
fn fixture() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let document = root.path().join("book");
    fs::create_dir_all(document.join("art")).expect("the art folder");

    fs::write(document.join("art").join("map.png"), b"PNG-ish bytes").expect("the image");
    fs::write(root.path().join("secret.txt"), b"not for the document").expect("the secret");

    (root, document)
}

#[test]
fn reads_an_image_inside_the_document_folder() {
    let (_root, document) = fixture();
    assert_eq!(
        read(&RealFs, &document, "art/map.png"),
        Ok(b"PNG-ish bytes".to_vec())
    );
}

#[test]
fn refuses_traversal_that_would_reach_a_real_file() {
    let (_root, document) = fixture();
    // The file is there and readable; the only thing stopping it is the check.
    assert!(document.join("../secret.txt").exists());
    assert_eq!(
        read(&RealFs, &document, "../secret.txt"),
        Err(Refusal::NotLocal)
    );
}

#[test]
fn refuses_a_missing_file() {
    let (_root, document) = fixture();
    assert_eq!(read(&RealFs, &document, "art/nope.png"), Err(Refusal::Missing));
}

#[test]
fn refuses_a_file_over_the_cap() {
    let (_root, document) = fixture();
    let big = document.join("art").join("huge.bin");
    // One byte past 20 MB, so the boundary is the thing being tested rather
    // than "very large" being the thing being tested.
    fs::write(&big, vec![0u8; 20 * 1024 * 1024 + 1]).expect("the oversized file");

    match read(&RealFs, &document, "art/huge.bin") {
        Err(Refusal::TooLarge { bytes }) => assert_eq!(bytes, 20 * 1024 * 1024 + 1),
        other => panic!("expected TooLarge, got {other:?}"),
    }
}

#[test]
fn accepts_a_file_exactly_at_the_cap() {
    let (_root, document) = fixture();
    let at = document.join("art").join("exact.bin");
    fs::write(&at, vec![7u8; 20 * 1024 * 1024]).expect("the file");

    assert!(read(&RealFs, &document, "art/exact.bin").is_ok());
}

/// A symlink is the case string checks cannot see.
///
/// Every component of `art/escape.png` is an ordinary name, so [`resolve`]
/// clears it and the only thing between the document and the file outside is
/// the containment check on the canonicalized path.
///
/// Ignored where it cannot be set up rather than silently passing: creating a
/// symlink on Windows needs Developer Mode or elevation, and a test that
/// quietly does nothing is worse than one that is visibly skipped.
#[test]
fn refuses_a_symlink_that_points_outside() {
    let (root, document) = fixture();
    let link = document.join("art").join("escape.png");
    let target = root.path().join("secret.txt");

    if !symlink(&target, &link) {
        eprintln!("skipped: this platform will not create a symlink here");
        return;
    }

    assert_eq!(read(&RealFs, &document, "art/escape.png"), Err(Refusal::NotLocal));
}

/// A symlink *inside* the folder is fine, which is what makes the check a
/// containment test rather than a ban on symlinks.
#[test]
fn allows_a_symlink_that_stays_inside() {
    let (_root, document) = fixture();
    let link = document.join("art").join("alias.png");
    let target = document.join("art").join("map.png");

    if !symlink(&target, &link) {
        eprintln!("skipped: this platform will not create a symlink here");
        return;
    }

    assert_eq!(
        read(&RealFs, &document, "art/alias.png"),
        Ok(b"PNG-ish bytes".to_vec())
    );
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn symlink(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}
