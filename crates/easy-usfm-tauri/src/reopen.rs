//! What to do when a document is opened — FILE-FIDELITY §4, P4.2.
//!
//! Two questions get asked together because the answer to one changes what the
//! other means. Who holds this file, and is there unsaved work from a session
//! that did not finish?
//!
//! - Held by a live foreign process → read-only, with **Open a copy** or **Take
//!   over**. Any snapshot belongs to *that* session and is not ours to offer.
//! - Held by a process that is gone → something crashed. If its snapshot
//!   differs from what is on disk, offer it, with a count of the lines.
//! - Free, but a snapshot is sitting there → a crash that lost its lock, or one
//!   from before locks existed. Same offer.
//!
//! **Recovery is always a choice, never automatic.** §4 says so twice, and it
//! matters most in the case that looks safest: silently restoring a snapshot
//! that happens to be stale would overwrite the file the user actually has,
//! with work they had already decided to abandon.

use std::path::Path;

use serde::Serialize;

use crate::fs::FileSystem;
use crate::lock::{inspect, Held, Owner};
use crate::recovery::{lines_differing, newest};

/// Everything the interface needs to decide what to show.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Reopen {
    /// Who holds the file, if anyone.
    pub held: Held,
    /// Present only when there is unsaved work worth offering back.
    pub recovery: Option<Recovery>,
}

/// An offer to restore work from a session that did not finish.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Recovery {
    /// When the snapshot was taken, for the prompt to name a time.
    pub taken_at: u64,
    /// How many lines differ from what is on disk.
    pub lines_differing: usize,
    /// The snapshot itself, so accepting costs no second round trip.
    pub text: String,
    /// Where the caret was.
    pub cursor: u32,
}

/// Answers both questions for one document.
pub fn examine<F: FileSystem>(
    filesystem: &F,
    root: &Path,
    canonical: &Path,
    us: &Owner,
    on_disk: Option<&str>,
) -> Reopen {
    let held = inspect(filesystem, root, canonical, us);

    // A live foreign instance's snapshot describes a session still in progress.
    // Offering it here would hand one window the other's half-finished work,
    // and accepting would then race that window's next save.
    if matches!(held, Held::Foreign { .. }) {
        return Reopen {
            held,
            recovery: None,
        };
    }

    let recovery = newest(filesystem, root, canonical).and_then(|(meta, text)| {
        // Nothing was outstanding when it was taken, so there is nothing to
        // recover -- the snapshot and the file agree by construction.
        if !meta.dirty {
            return None;
        }

        let differing = lines_differing(&text, on_disk.unwrap_or(""));
        // Identical to what is on disk. Offering it would ask the user a
        // question with no wrong answer and no right one either, which trains
        // people to dismiss the prompt that matters.
        if differing == 0 {
            return None;
        }

        Some(Recovery {
            taken_at: meta.taken_at,
            lines_differing: differing,
            text,
            cursor: meta.cursor,
        })
    });

    Reopen { held, recovery }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::RealFs;
    use crate::lock::{now_ms, take};
    use crate::recovery::{snapshot, Meta};
    use easy_usfm_core::Eol;
    use std::path::PathBuf;

    fn meta(at: u64, dirty: bool) -> Meta {
        Meta {
            path: Some("/books/GEN.usfm".to_string()),
            bom: false,
            eol: Eol::Lf,
            final_newline: true,
            cursor: 12,
            dirty,
            pid: 1,
            app_version: "0.1.0".to_string(),
            taken_at: at,
        }
    }

    fn document() -> &'static Path {
        Path::new("/books/GEN.usfm")
    }

    fn dead() -> Owner {
        Owner {
            pid: 4_294_967_294,
            started_at: 1,
            host: Owner::current(0).host,
            app_version: "0.1.0".to_string(),
        }
    }

    #[test]
    fn a_clean_start_offers_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());

        let seen = examine(&RealFs, temp.path(), document(), &us, Some("on disk"));

        assert_eq!(seen.held, Held::Free);
        assert!(seen.recovery.is_none());
    }

    #[test]
    fn a_crash_with_unsaved_work_offers_it_with_a_count() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        take(&RealFs, temp.path(), document(), &dead()).unwrap();
        snapshot(
            &RealFs,
            temp.path(),
            document(),
            "one\ntwo changed\nthree\nfour",
            &meta(1_700_000_000_000, true),
        )
        .unwrap();

        let seen = examine(
            &RealFs,
            temp.path(),
            document(),
            &us,
            Some("one\ntwo\nthree"),
        );

        assert!(matches!(seen.held, Held::Crashed { .. }));
        let recovery = seen.recovery.expect("work to recover");
        // Line two changed, and line four is new.
        assert_eq!(recovery.lines_differing, 2);
        assert_eq!(recovery.taken_at, 1_700_000_000_000);
        assert_eq!(recovery.cursor, 12);
    }

    #[test]
    fn a_snapshot_matching_the_file_is_not_offered() {
        // The save landed and the clear did not, or the snapshot was taken and
        // then the same text was saved. Asking about it trains people to
        // dismiss the prompt that matters.
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        take(&RealFs, temp.path(), document(), &dead()).unwrap();
        snapshot(
            &RealFs,
            temp.path(),
            document(),
            "same\ntext",
            &meta(1, true),
        )
        .unwrap();

        let seen = examine(&RealFs, temp.path(), document(), &us, Some("same\ntext"));

        assert!(seen.recovery.is_none());
    }

    #[test]
    fn a_snapshot_of_saved_work_is_not_offered() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        snapshot(&RealFs, temp.path(), document(), "text", &meta(1, false)).unwrap();

        let seen = examine(&RealFs, temp.path(), document(), &us, Some("different"));

        assert!(seen.recovery.is_none());
    }

    #[test]
    fn a_live_instances_work_is_not_offered_to_the_second_window() {
        // Its snapshot describes a session still in progress. Handing it over
        // would give one window the other's half-finished work, and accepting
        // would race that window's next save.
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        let other = Owner {
            pid: std::process::id(),
            started_at: 1,
            host: us.host.clone(),
            app_version: "0.1.0".to_string(),
        };
        take(&RealFs, temp.path(), document(), &other).unwrap();
        snapshot(&RealFs, temp.path(), document(), "theirs", &meta(1, true)).unwrap();

        let seen = examine(&RealFs, temp.path(), document(), &us, Some("ours"));

        assert!(matches!(seen.held, Held::Foreign { .. }));
        assert!(seen.recovery.is_none());
    }

    #[test]
    fn a_snapshot_with_no_lock_at_all_is_still_offered() {
        // A crash that lost its lock, or one from before locks existed. The
        // work is no less real for the bookkeeping having gone.
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        snapshot(&RealFs, temp.path(), document(), "recovered", &meta(1, true)).unwrap();

        let seen = examine(&RealFs, temp.path(), document(), &us, Some("on disk"));

        assert_eq!(seen.held, Held::Free);
        assert!(seen.recovery.is_some());
    }

    #[test]
    fn a_file_that_no_longer_exists_still_offers_its_work() {
        // The file was deleted while the editor was gone. Everything the
        // snapshot holds is a difference, and it is the only copy left.
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        snapshot(&RealFs, temp.path(), document(), "a\nb\nc", &meta(1, true)).unwrap();

        let seen = examine(&RealFs, temp.path(), document(), &us, None);

        assert_eq!(seen.recovery.map(|r| r.lines_differing), Some(3));
    }

    #[test]
    fn the_newest_generation_is_the_one_offered() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        for at in [10u64, 20, 30] {
            snapshot(
                &RealFs,
                temp.path(),
                document(),
                &format!("generation {at}"),
                &meta(at, true),
            )
            .unwrap();
        }

        let seen = examine(&RealFs, temp.path(), document(), &us, Some("disk"));

        assert_eq!(seen.recovery.map(|r| r.text), Some("generation 30".into()));
    }

    #[test]
    fn a_generation_interrupted_between_its_two_writes_is_skipped() {
        // A snapshot is two files. A crash between them leaves a generation
        // that is newest and unusable; the one before it is still good.
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        snapshot(&RealFs, temp.path(), document(), "complete", &meta(10, true)).unwrap();

        let torn: PathBuf = crate::recovery::directory_for(temp.path(), document()).join("20");
        std::fs::create_dir_all(&torn).unwrap();
        std::fs::write(torn.join("snapshot.usfm"), b"half written").unwrap();

        let seen = examine(&RealFs, temp.path(), document(), &us, Some("disk"));

        assert_eq!(seen.recovery.map(|r| r.text), Some("complete".into()));
    }
}
