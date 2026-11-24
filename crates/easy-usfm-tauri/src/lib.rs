//! The desktop editor: easy-usfm's own window around the shell.
//!
//! Everything native lives in `tauri-plugin-easy-usfm` — ARCHITECTURE §2's
//! dialogs, reading, atomic writing, watching, recovery — because BibleCompose
//! opens the same editor in its own process and has to write a book exactly as
//! this application would. What is left here is what only the standalone
//! application has: its window, the file a double-click hands it, and the
//! update check.

/// Builds and runs the application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Hands an external link to the OS handler (SECURITY 2). Never the
        // webview -- a link opened there runs in this application's origin.
        .plugin(tauri_plugin_opener::init())
        // The window from `tauri.conf.json` is the editor, and a file named on
        // the command line -- a file association, a double-click -- is its to
        // open.
        .plugin(
            tauri_plugin_easy_usfm::Builder::new()
                .adopt("main")
                .launch_from_args()
                .build(),
        )
        .run(tauri::generate_context!())
        .expect("the application failed to start");
}
