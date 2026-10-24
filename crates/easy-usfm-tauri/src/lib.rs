//! The desktop shell.
//!
//! ARCHITECTURE §2: native Rust handles only what browsers cannot — dialogs,
//! reading, atomic writing, watching, recovery. **No parsing happens here.**
//! There is one engine on every target and it runs in a worker, so this crate
//! stays a file-access layer however tempting it becomes to call the parser it
//! already links against.
//!
//! Still ahead of it: recovery and watching (Phase 4), and per-platform menus
//! (P6.1).

pub mod document;
pub mod figure;
pub mod fs;
pub mod lock;
pub mod menu;
pub mod recovery;
pub mod reopen;
pub mod save;
pub mod updates;
pub mod watch;

use std::path::PathBuf;

use document::Documents;

/// The file this window was launched to open, if it was launched to open one.
///
/// BibleCompose opens a book's USFM by starting this editor on it, and a file
/// association does the same from the desktop. Either way the path arrives as
/// an argument.
///
/// **The interface asks for it rather than being told.** An event emitted while
/// the shell is setting up fires before the frontend is listening, and the file
/// is then simply lost — so it is held here until something with a window asks.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Launched(Option<PathBuf>);

impl Launched {
    /// The first argument that names a file which exists.
    ///
    /// Not `args[1]`. A webview adds arguments of its own on some platforms, a
    /// flag is never a path, and a publisher may open a file whose name this
    /// has no opinion about. So the test is that the thing exists and is a
    /// file — which also means a path typed wrongly opens an empty editor
    /// rather than an error nobody asked for.
    fn from_args<I>(args: I) -> Launched
    where
        I: IntoIterator<Item = String>,
    {
        Launched(
            args.into_iter()
                .filter(|a| !a.starts_with('-'))
                .map(PathBuf::from)
                .find(|p| p.is_file()),
        )
    }

    /// What this process was started with.
    fn from_env() -> Launched {
        // `args_os` and not `args`: the latter panics on an argument that is
        // not Unicode, and a path handed over by the operating system is not
        // this application's to validate by crashing. One that cannot be a
        // Rust string is skipped, and the editor opens empty.
        Launched::from_args(
            std::env::args_os()
                .skip(1)
                .filter_map(|a| a.into_string().ok()),
        )
    }
}

/// The path this window was launched on, for the interface to open.
#[tauri::command]
fn launched_with(launched: tauri::State<'_, Launched>) -> Option<String> {
    launched
        .0
        .as_ref()
        .and_then(|p| p.to_str())
        .map(str::to_owned)
}

/// The engine version, and the round trip that proves the two halves are
/// talking.
#[tauri::command]
fn engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Builds and runs the application.
///
/// Separated from `main` so integration tests can reach everything this crate
/// does without opening a window — which matters most for the save ladder,
/// where the interesting cases are filesystem failures rather than clicks.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Hands an external link to the OS handler (SECURITY 2). Never the
        // webview -- a link opened there runs in this application's origin.
        .plugin(tauri_plugin_opener::init())
        .manage(Documents::default())
        // Fixed for the process's lifetime, so a pid recorded in a lock can be
        // told apart from the same number reused by something else (P4.2).
        .manage(recovery::Started(lock::now_ms()))
        .manage(watch::FileWatch::default())
        // Read once, here, because `setup` runs after this and the arguments
        // are the same for the life of the process.
        .manage(Launched::from_env())
        .setup(|app| {
            // Empty to begin with; the interface pushes its recent list once
            // it has read its settings.
            menu::install(app.handle(), &[])?;

            // FILE-FIDELITY 4: "directories older than 30 days pruned at
            // startup". Off the main thread -- it is a directory walk over
            // however much has accumulated, and nothing is waiting on it.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                use tauri::Manager;
                if let Ok(data) = handle.path().app_data_dir() {
                    recovery::prune(
                        &fs::RealFs,
                        &data.join("recovery"),
                        std::time::SystemTime::now(),
                    );
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_version,
            menu::set_recent_files,
            document::new_document,
            document::open_document,
            document::save_document,
            document::close_document,
            figure::read_figure,
            recovery::snapshot_document,
            recovery::clear_recovery,
            recovery::examine_document,
            recovery::take_lock,
            recovery::release_lock,
            watch::watch_document,
            watch::unwatch_document,
            updates::updates_possible,
            updates::check_for_update,
            launched_with,
        ])
        .run(tauri::generate_context!())
        .expect("the application failed to start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_reported() {
        assert_eq!(engine_version(), env!("CARGO_PKG_VERSION"));
    }

    /// **The file to open is the first argument that is one.**
    ///
    /// A flag is skipped because a webview adds its own, and a path that is
    /// not there is skipped because an editor opening empty beats an editor
    /// reporting something the person never typed.
    #[test]
    fn the_launch_file_is_the_first_argument_that_is_a_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let book = dir.path().join("41_MAT.usfm");
        std::fs::write(&book, "\\id MAT\n").expect("write");
        let other = dir.path().join("42_MRK.usfm");
        std::fs::write(&other, "\\id MRK\n").expect("write");
        let as_str = |p: &std::path::Path| p.to_str().expect("UTF-8").to_owned();

        // The plain case: one path, handed over by a double-click.
        assert_eq!(
            Launched::from_args([as_str(&book)]),
            Launched(Some(book.clone()))
        );

        // Flags are not paths, whoever added them.
        assert_eq!(
            Launched::from_args(["--webview-flag".to_owned(), as_str(&book)]),
            Launched(Some(book.clone()))
        );

        // The first that exists wins, so a stray argument before it is passed
        // over rather than opened.
        assert_eq!(
            Launched::from_args([as_str(&dir.path().join("nope.usfm")), as_str(&other)]),
            Launched(Some(other))
        );

        // A directory is not a file, and neither is nothing.
        assert_eq!(Launched::from_args([as_str(dir.path())]), Launched(None));
        assert_eq!(Launched::from_args([] as [String; 0]), Launched(None));
    }

    /// And the command hands back exactly what was found.
    #[test]
    fn the_command_reports_the_launch_file() {
        assert_eq!(Launched::default().0, None);
        let dir = tempfile::tempdir().expect("temp dir");
        let book = dir.path().join("01_GEN.usfm");
        std::fs::write(&book, "\\id GEN\n").expect("write");
        let found = Launched::from_args([book.to_str().expect("UTF-8").to_owned()]);
        assert_eq!(found.0.as_deref(), Some(book.as_path()));
    }
}
