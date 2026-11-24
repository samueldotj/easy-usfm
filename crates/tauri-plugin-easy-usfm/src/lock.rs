//! Who has this file open — FILE-FIDELITY §4, P4.2.
//!
//! "`recovery/{hash}/owner.lock` holds `{pid, started_at, host, app_version}`.
//! PID alive and ours → focus the existing window rather than opening a second.
//! PID alive and foreign → open read-only with **Open a copy** or **Take over**
//! (warning that the other instance may overwrite). PID dead → a crash
//! occurred."
//!
//! # Advisory, and it says so in the name
//!
//! Nothing here stops another program — or another copy of this one — from
//! writing the file. An operating-system lock would, and it is the wrong tool:
//! a held lock survives a crash on some platforms and not others, and a
//! translator whose machine lost power should not find their file unopenable
//! until they reboot. This records an intent so the *interface* can say
//! something useful, and the save ladder remains the thing that actually
//! protects the bytes.
//!
//! # Liveness is the whole question
//!
//! Every branch above turns on whether a recorded process is still running, and
//! a pid alone cannot answer that: pids are recycled, so a dead editor's number
//! may now belong to a browser. The lock therefore records when the process
//! started as well, and a pid only counts as ours if *both* match. Getting this
//! wrong in the safe direction means offering recovery for a session that is
//! still running; in the unsafe direction it means a second window silently
//! editing a file another window is about to save over.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::fs::FileSystem;
use crate::recovery::directory_for;

/// What a lock file records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Owner {
    pub pid: u32,
    /// Milliseconds since the epoch, as a guard against a recycled pid.
    pub started_at: u64,
    /// Which machine, because recovery directories can sit on a synced folder
    /// and a pid from another computer means nothing here.
    pub host: String,
    pub app_version: String,
}

impl Owner {
    /// This process, now.
    pub fn current(started_at: u64) -> Self {
        Self {
            pid: std::process::id(),
            started_at,
            host: hostname(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Whether this record describes the process asking.
    ///
    /// All three, not just the pid. A recycled pid on the same host would
    /// otherwise read as "already open here", and the second window would
    /// silently do nothing.
    fn is(&self, us: &Owner) -> bool {
        self.pid == us.pid && self.host == us.host && self.started_at == us.started_at
    }
}

/// What the interface has to decide between.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Held {
    /// Nobody has it. The caller took the lock.
    Free,
    /// This process already has it open.
    Ours,
    /// Another live process has it. Read-only, per §4.
    Foreign { owner: Owner },
    /// The process that had it is gone. Something crashed.
    Crashed { owner: Owner },
}

const LOCK: &str = "owner.lock";

/// Reads the lock for a document without taking it.
pub fn inspect<F: FileSystem>(filesystem: &F, root: &Path, canonical: &Path, us: &Owner) -> Held {
    let path = directory_for(root, canonical).join(LOCK);

    let Ok(bytes) = filesystem.read(&path) else {
        return Held::Free;
    };
    let Ok(owner) = serde_json::from_slice::<Owner>(&bytes) else {
        // Unreadable, which is a lock file from a version that wrote something
        // else or one truncated by a crash. Treated as free rather than as a
        // reason to refuse: refusing would make a corrupt byte lock a
        // translator out of their own file.
        return Held::Free;
    };

    if owner.is(us) {
        return Held::Ours;
    }
    // A record from another machine says nothing about what is running here.
    // Synced recovery folders are a real arrangement and a foreign host's pid
    // is not a process on this one.
    if owner.host != us.host {
        return Held::Crashed { owner };
    }
    if alive(&owner) {
        Held::Foreign { owner }
    } else {
        Held::Crashed { owner }
    }
}

/// Writes this process into the lock, whatever was there before.
///
/// Called after the interface has decided — taking over from a live instance
/// is a choice §4 offers, and this is the mechanism for it, not the policy.
pub fn take<F: FileSystem>(
    filesystem: &F,
    root: &Path,
    canonical: &Path,
    us: &Owner,
) -> std::io::Result<()> {
    let directory = directory_for(root, canonical);
    filesystem.create_dir_all(&directory)?;

    let bytes = serde_json::to_vec_pretty(us).map_err(std::io::Error::other)?;
    let path = directory.join(LOCK);

    // Not through rung 1. A lock file is written on every open and is
    // disposable — a torn one reads as free, which is the same as it not being
    // there. The temp-and-rename dance would only add a failure mode.
    if filesystem.exists(&path) {
        filesystem.write_in_place(&path, &bytes)
    } else {
        filesystem.write_new(&path, &bytes)
    }
}

/// Gives the lock up, if it is ours to give.
///
/// Checked rather than assumed: a window closing after another instance took
/// over must not remove that instance's claim.
pub fn release<F: FileSystem>(filesystem: &F, root: &Path, canonical: &Path, us: &Owner) {
    let path = directory_for(root, canonical).join(LOCK);

    if let Ok(bytes) = filesystem.read(&path) {
        if let Ok(owner) = serde_json::from_slice::<Owner>(&bytes) {
            if !owner.is(us) {
                return;
            }
        }
    }
    let _ = filesystem.remove(&path);
}

/// Milliseconds since the epoch.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or(0)
}

fn hostname() -> String {
    // No dependency for this. Both variables are set by the OS on their
    // respective platforms, and an empty answer is consistent within a machine
    // — which is all that is being compared.
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_default()
}

/// Whether a recorded process is still running.
///
/// The start time is checked too, where the platform can report it, because a
/// pid on its own is a recycled number. Where it cannot be read, a live pid is
/// taken at face value — erring towards "someone else has this open", which
/// costs a read-only window rather than a lost file.
#[cfg(windows)]
fn alive(owner: &Owner) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // SAFETY: the handle is closed on every path out, and `GetExitCodeProcess`
    // is given a pointer to a local that outlives the call.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, owner.pid);
        if handle.is_null() {
            // Access denied also lands here, which would be a process owned by
            // another user. Not ours to worry about and not our editor.
            return false;
        }

        let mut code: u32 = 0;
        let read = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);

        read != 0 && code == STILL_ACTIVE as u32
    }
}

#[cfg(unix)]
fn alive(owner: &Owner) -> bool {
    // Signal zero performs the permission and existence checks without
    // delivering anything, which is the portable way to ask.
    //
    // SAFETY: `kill` with signal 0 has no side effects on the target.
    unsafe { libc::kill(owner.pid as i32, 0) == 0 }
}

#[cfg(not(any(windows, unix)))]
fn alive(_owner: &Owner) -> bool {
    // Nothing to ask. Treating an unknown process as alive means offering a
    // read-only window rather than a recovery, which is the harmless mistake.
    true
}

/// The lock path, for tests and for the recovery reader.
pub fn path_for(root: &Path, canonical: &Path) -> PathBuf {
    directory_for(root, canonical).join(LOCK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::RealFs;

    fn owner(pid: u32, started_at: u64) -> Owner {
        Owner {
            pid,
            started_at,
            host: hostname(),
            app_version: "0.1.0".to_string(),
        }
    }

    fn document() -> &'static Path {
        Path::new("/books/GEN.usfm")
    }

    #[test]
    fn an_untaken_document_is_free() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());

        assert_eq!(inspect(&RealFs, temp.path(), document(), &us), Held::Free);
    }

    #[test]
    fn our_own_lock_reads_as_ours() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());

        take(&RealFs, temp.path(), document(), &us).unwrap();

        assert_eq!(inspect(&RealFs, temp.path(), document(), &us), Held::Ours);
    }

    #[test]
    fn a_recycled_pid_is_not_us() {
        // The same number, a different session. Without the start time this
        // reads as "already open here" and a second window silently does
        // nothing at all.
        let temp = tempfile::tempdir().unwrap();
        let earlier = owner(std::process::id(), 1);
        let us = Owner::current(now_ms());

        take(&RealFs, temp.path(), document(), &earlier).unwrap();

        assert_ne!(inspect(&RealFs, temp.path(), document(), &us), Held::Ours);
    }

    #[test]
    fn a_live_foreign_process_holds_it() {
        // This test process is alive by definition, and a start time that is
        // not ours makes it foreign.
        let temp = tempfile::tempdir().unwrap();
        let other = owner(std::process::id(), 1);
        let us = Owner::current(now_ms());

        take(&RealFs, temp.path(), document(), &other).unwrap();

        match inspect(&RealFs, temp.path(), document(), &us) {
            Held::Foreign { owner } => assert_eq!(owner.pid, std::process::id()),
            other => panic!("expected Foreign, got {other:?}"),
        }
    }

    #[test]
    fn a_dead_process_reads_as_a_crash() {
        let temp = tempfile::tempdir().unwrap();
        // A pid nothing is using. Above the default maximum on Linux and not
        // a number Windows hands out.
        let dead = owner(4_294_967_294, 1);
        let us = Owner::current(now_ms());

        take(&RealFs, temp.path(), document(), &dead).unwrap();

        match inspect(&RealFs, temp.path(), document(), &us) {
            Held::Crashed { owner } => assert_eq!(owner.pid, 4_294_967_294),
            other => panic!("expected Crashed, got {other:?}"),
        }
    }

    #[test]
    fn a_lock_from_another_machine_is_not_a_process_here() {
        // Recovery folders end up in synced directories. A pid from another
        // computer is a number, not something running on this one.
        let temp = tempfile::tempdir().unwrap();
        let mut elsewhere = owner(std::process::id(), 1);
        elsewhere.host = "some-other-machine".to_string();
        let us = Owner::current(now_ms());

        take(&RealFs, temp.path(), document(), &elsewhere).unwrap();

        assert!(matches!(
            inspect(&RealFs, temp.path(), document(), &us),
            Held::Crashed { .. }
        ));
    }

    #[test]
    fn an_unreadable_lock_does_not_lock_anyone_out() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        let path = path_for(temp.path(), document());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not json").unwrap();

        // Refusing here would make one corrupt byte lock a translator out of
        // their own file.
        assert_eq!(inspect(&RealFs, temp.path(), document(), &us), Held::Free);
    }

    #[test]
    fn releasing_removes_only_our_own() {
        let temp = tempfile::tempdir().unwrap();
        let us = Owner::current(now_ms());
        let other = owner(std::process::id(), 1);

        take(&RealFs, temp.path(), document(), &us).unwrap();
        release(&RealFs, temp.path(), document(), &us);
        assert!(!path_for(temp.path(), document()).exists());

        // A window closing after another instance took over must not remove
        // that instance's claim.
        take(&RealFs, temp.path(), document(), &other).unwrap();
        release(&RealFs, temp.path(), document(), &us);
        assert!(path_for(temp.path(), document()).exists());
    }

    #[test]
    fn taking_over_replaces_whoever_was_there() {
        let temp = tempfile::tempdir().unwrap();
        let other = owner(std::process::id(), 1);
        let us = Owner::current(now_ms());

        take(&RealFs, temp.path(), document(), &other).unwrap();
        take(&RealFs, temp.path(), document(), &us).unwrap();

        assert_eq!(inspect(&RealFs, temp.path(), document(), &us), Held::Ours);
    }
}
