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
pub mod fs;
pub mod save;

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
        .manage(Documents::default())
        .invoke_handler(tauri::generate_handler![
            engine_version,
            document::new_document,
            document::open_document,
            document::save_document,
            document::close_document,
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
