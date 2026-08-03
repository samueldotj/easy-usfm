//! The desktop shell.
//!
//! ARCHITECTURE §2: native Rust handles only what browsers cannot — dialogs,
//! reading, atomic writing, watching, recovery. **No parsing happens here.**
//! There is one engine on every target and it runs in a worker, so this crate
//! stays a file-access layer however tempting it becomes to call the parser it
//! already links against.
//!
//! Almost all of this crate's work is still ahead of it: the atomic save
//! ladder (P1.5–P1.7), fault injection (P1.8), the document lifecycle (P1.10),
//! and recovery and watching (Phase 4).

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
        .invoke_handler(tauri::generate_handler![engine_version])
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
