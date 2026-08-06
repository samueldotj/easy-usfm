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

use document::Documents;

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
}
