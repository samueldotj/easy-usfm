//! Noticing that a file changed underneath us — FILE-FIDELITY §3, P4.3.
//!
//! "The watcher must ignore our own writes **by content hash, not timing** —
//! timing alone is unreliable on network filesystems where events arrive
//! seconds late, and on sync clients that rewrite files after upload."
//!
//! ```text
//! 1. Debounce 250 ms (editors and sync clients emit event storms).
//! 2. Read the file and hash it.
//! 3. hash == last_known_hash             → ignore silently.
//! 4. hash matches a SaveEpoch < 30 s old → ignore, drop the epoch.
//! 5. otherwise                           → genuine external change.
//! ```
//!
//! "Without this, every save triggers 'this file changed on disk', which trains
//! users to dismiss the prompt that matters."
//!
//! # The two hashes are not the same question
//!
//! Step 3 asks "is the file what we last read or wrote?" and answers the common
//! case: nothing really changed, some tool touched the mtime. Step 4 asks "did
//! *we* produce these bytes recently?" and exists for the save ladder's slower
//! rungs, where a write lands as several filesystem events and a sync client may
//! rewrite the file afterwards with identical content. An epoch is consumed when
//! it matches, so a genuine external edit that happens to restore the bytes we
//! once wrote is still reported — the second time.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// FILE-FIDELITY §3, step 1.
pub const DEBOUNCE: Duration = Duration::from_millis(250);

/// How long a write of ours stays recognisable.
const EPOCH_LIFETIME: Duration = Duration::from_secs(30);

/// How many of our own writes are remembered at once.
///
/// Small on purpose. Epochs exist to cover a single save arriving as several
/// events; a queue long enough to hold a session's worth of saves would keep
/// matching an external edit that reverted the file to an old state.
const EPOCHS: usize = 8;

/// What a watcher event turned out to mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The file is what we last read or wrote.
    Unchanged,
    /// These are bytes we wrote, arriving late.
    OurOwnWrite,
    /// Someone else changed it.
    External,
    /// It is not there any more.
    Gone,
}

/// One write of ours, recognisable for a while.
#[derive(Debug, Clone, Copy)]
struct Epoch {
    hash: [u8; 32],
    at: Instant,
}

/// What the shell knows about a file it is watching.
#[derive(Debug)]
pub struct Watched {
    /// The bytes we last read from or wrote to this file.
    known: [u8; 32],
    /// Writes of ours that may still be echoing back.
    epochs: VecDeque<Epoch>,
}

impl Watched {
    /// Begins watching a file whose contents we know.
    pub fn new(known: [u8; 32]) -> Self {
        Self {
            known,
            epochs: VecDeque::new(),
        }
    }

    /// Records that we have just written these bytes.
    ///
    /// Both the known hash *and* an epoch. The known hash covers the ordinary
    /// case where the event arrives before anything else happens; the epoch
    /// covers the file being written again — by a sync client, or by the save
    /// ladder's copy-back rung — between our write and the event.
    pub fn wrote(&mut self, hash: [u8; 32], now: Instant) {
        self.known = hash;
        self.epochs.push_back(Epoch { hash, at: now });
        if self.epochs.len() > EPOCHS {
            self.epochs.pop_front();
        }
    }

    /// Records what we just read, which is now what we believe is there.
    pub fn read(&mut self, hash: [u8; 32]) {
        self.known = hash;
    }

    /// What an event about this file means.
    ///
    /// `None` for a file that has gone. Deletion is not a content change and
    /// cannot be hashed, and §3 gives it its own outcome: the buffer is kept
    /// and the user is told Save will recreate it.
    pub fn judge(&mut self, bytes: Option<&[u8]>, now: Instant) -> Verdict {
        let Some(bytes) = bytes else {
            return Verdict::Gone;
        };
        let hash = *blake3::hash(bytes).as_bytes();

        if hash == self.known {
            return Verdict::Unchanged;
        }

        // Expired epochs first, so a match cannot be made against a write from
        // half an hour ago that the user has long since forgotten.
        self.epochs
            .retain(|epoch| now.duration_since(epoch.at) < EPOCH_LIFETIME);

        if let Some(index) = self.epochs.iter().position(|epoch| epoch.hash == hash) {
            // Consumed. A later event with the same bytes is a real change --
            // someone put back what we once wrote -- and must be reported.
            self.epochs.remove(index);
            self.known = hash;
            return Verdict::OurOwnWrite;
        }

        self.known = hash;
        Verdict::External
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(bytes: &[u8]) -> [u8; 32] {
        *blake3::hash(bytes).as_bytes()
    }

    #[test]
    fn the_file_we_last_read_is_not_a_change() {
        let mut watched = Watched::new(hash(b"one"));

        assert_eq!(watched.judge(Some(b"one"), Instant::now()), Verdict::Unchanged);
    }

    #[test]
    fn our_own_save_never_prompts() {
        // The acceptance criterion, and the reason the whole mechanism exists:
        // "without this, every save triggers 'this file changed on disk', which
        // trains users to dismiss the prompt that matters."
        let mut watched = Watched::new(hash(b"before"));
        let now = Instant::now();

        watched.wrote(hash(b"after"), now);

        assert_eq!(watched.judge(Some(b"after"), now), Verdict::Unchanged);
    }

    #[test]
    fn a_save_rewritten_by_something_else_is_still_ours() {
        // The case timing alone gets wrong. A sync client rewrites the file
        // after upload, so the event arrives describing bytes that are ours
        // but are no longer what we think is there.
        let mut watched = Watched::new(hash(b"before"));
        let now = Instant::now();

        watched.wrote(hash(b"ours"), now);
        // Something else touched it in between, and we noticed.
        watched.read(hash(b"interloper"));

        assert_eq!(watched.judge(Some(b"ours"), now), Verdict::OurOwnWrite);
    }

    #[test]
    fn someone_elses_edit_always_prompts() {
        let mut watched = Watched::new(hash(b"ours"));

        assert_eq!(
            watched.judge(Some(b"theirs"), Instant::now()),
            Verdict::External
        );
    }

    #[test]
    fn an_epoch_is_consumed_by_the_event_it_explains() {
        // Someone putting back exactly what we wrote earlier is a real change.
        // The first event is ours; a second one with the same bytes is not.
        let mut watched = Watched::new(hash(b"before"));
        let now = Instant::now();

        watched.wrote(hash(b"ours"), now);
        // Something moved the file on before the event arrived, so step 3 no
        // longer answers and the epoch is what explains it.
        watched.read(hash(b"interloper"));
        assert_eq!(watched.judge(Some(b"ours"), now), Verdict::OurOwnWrite);

        watched.read(hash(b"something else"));
        assert_eq!(watched.judge(Some(b"ours"), now), Verdict::External);
    }

    #[test]
    fn an_old_write_stops_explaining_anything() {
        let mut watched = Watched::new(hash(b"before"));
        let now = Instant::now();

        watched.wrote(hash(b"ours"), now);
        watched.read(hash(b"something else"));

        // Half an hour later, the same bytes appear. That is somebody
        // restoring a backup, not our save arriving late.
        let much_later = now + EPOCH_LIFETIME + Duration::from_secs(1);
        assert_eq!(watched.judge(Some(b"ours"), much_later), Verdict::External);
    }

    #[test]
    fn only_a_few_writes_are_remembered() {
        let mut watched = Watched::new(hash(b"start"));
        let now = Instant::now();

        for round in 0..EPOCHS + 4 {
            watched.wrote(hash(format!("save {round}").as_bytes()), now);
        }
        watched.read(hash(b"something else"));

        // The oldest have fallen out, so reverting to one of them is reported
        // rather than silently swallowed.
        assert_eq!(
            watched.judge(Some(b"save 0"), now),
            Verdict::External,
            "an old save should no longer explain an event"
        );
        assert_eq!(
            watched.judge(Some(format!("save {}", EPOCHS + 3).as_bytes()), now),
            Verdict::OurOwnWrite
        );
    }

    #[test]
    fn a_deleted_file_is_its_own_outcome() {
        // Not a content change and not hashable. §3 keeps the buffer and says
        // Save will recreate it.
        let mut watched = Watched::new(hash(b"anything"));

        assert_eq!(watched.judge(None, Instant::now()), Verdict::Gone);
    }

    #[test]
    fn a_reported_change_becomes_what_we_believe() {
        // Otherwise every subsequent event about an unchanged file would be
        // reported again, and the bar would never go away.
        let mut watched = Watched::new(hash(b"ours"));
        let now = Instant::now();

        assert_eq!(watched.judge(Some(b"theirs"), now), Verdict::External);
        assert_eq!(watched.judge(Some(b"theirs"), now), Verdict::Unchanged);
    }
}

// --------------------------------------------------------------- watching ---

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Mutex;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use serde::Serialize;

/// What the interface is told when a file changes underneath it.
#[derive(Debug, Clone, Serialize)]
pub struct Changed {
    /// `"external"` or `"gone"`. The quiet verdicts are never sent.
    pub kind: &'static str,
    pub path: String,
    /// The file's new contents, so a clean reload costs no second round trip.
    /// Absent when the file is gone.
    pub text: Option<String>,
}

/// The one file this window is watching.
///
/// A watcher is held rather than recreated per event because dropping it stops
/// the watch, and one file at a time is what PRODUCT §3 asks for: one file per
/// window.
#[derive(Default)]
pub struct FileWatch(Mutex<Option<Active>>);

pub struct Active {
    path: PathBuf,
    /// Kept alive; dropping it ends the watch.
    _watcher: RecommendedWatcher,
    state: std::sync::Arc<Mutex<Watched>>,
}

impl FileWatch {
    /// Tells the watcher what we just wrote, so the event it causes is quiet.
    ///
    /// Called from the save path. FILE-FIDELITY §3's whole point is that this
    /// is by content, not timing: the hash goes in, and whenever the event
    /// arrives — a second later on a local disk, ten on a network mount — it
    /// is recognised.
    pub fn wrote(&self, path: &Path, hash: [u8; 32]) {
        if let Ok(held) = self.0.lock() {
            if let Some(active) = held.as_ref() {
                if active.path == path {
                    if let Ok(mut state) = active.state.lock() {
                        state.wrote(hash, Instant::now());
                    }
                }
            }
        }
    }
}

/// Starts watching a file, replacing whatever was being watched before.
///
/// The parent directory rather than the file itself. Editors and sync clients
/// replace files by renaming a temporary over them — which is what this
/// application's own save ladder does — and a watch on the inode goes with the
/// file that was replaced, so the next change is never seen.
#[tauri::command]
pub fn watch_document<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
    watch: tauri::State<'_, FileWatch>,
) -> Result<(), String> {
    use tauri::Emitter;

    let target = PathBuf::from(&path);
    let directory = target
        .parent()
        .ok_or_else(|| "that path has no directory".to_string())?
        .to_path_buf();

    let known = std::fs::read(&target)
        .map(|bytes| *blake3::hash(&bytes).as_bytes())
        .unwrap_or([0; 32]);
    let state = std::sync::Arc::new(Mutex::new(Watched::new(known)));

    let (sender, receiver) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = sender.send(event);
    })
    .map_err(|error| error.to_string())?;

    watcher
        .watch(&directory, RecursiveMode::NonRecursive)
        .map_err(|error| error.to_string())?;

    let handle = app.clone();
    let watched = state.clone();
    let file = target.clone();
    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            let Ok(event) = event else { continue };
            if !event.paths.iter().any(|touched| touched == &file) {
                continue;
            }

            // Step 1: editors and sync clients emit event storms, and a burst
            // of twenty events about one save should be answered once.
            std::thread::sleep(DEBOUNCE);
            while receiver.try_recv().is_ok() {}

            let bytes = std::fs::read(&file).ok();
            let verdict = match watched.lock() {
                Ok(mut state) => state.judge(bytes.as_deref(), Instant::now()),
                Err(_) => continue,
            };

            let kind = match verdict {
                // Steps 3 and 4. Silence is the whole point of them.
                Verdict::Unchanged | Verdict::OurOwnWrite => continue,
                Verdict::External => "external",
                Verdict::Gone => "gone",
            };

            let _ = handle.emit(
                "file-changed",
                Changed {
                    kind,
                    path: file.to_string_lossy().to_string(),
                    text: bytes.and_then(|bytes| String::from_utf8(bytes).ok()),
                },
            );
        }
    });

    if let Ok(mut held) = watch.0.lock() {
        *held = Some(Active {
            path: target,
            _watcher: watcher,
            state,
        });
    }
    Ok(())
}

/// Stops watching, on a close or when moving to another file.
#[tauri::command]
pub fn unwatch_document(watch: tauri::State<'_, FileWatch>) {
    if let Ok(mut held) = watch.0.lock() {
        *held = None;
    }
}
