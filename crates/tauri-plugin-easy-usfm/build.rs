//! Generates an `allow-` and `deny-` permission per command, which the
//! `default` set in `permissions/default.toml` collects. A command missing
//! from this list cannot be granted, so it cannot be called: the list is the
//! plugin's whole surface.

const COMMANDS: &[&str] = &[
    "engine_version",
    "shell_profile",
    "launched_with",
    "set_recent_files",
    "new_document",
    "open_document",
    "save_document",
    "close_document",
    "read_figure",
    "snapshot_document",
    "clear_recovery",
    "examine_document",
    "take_lock",
    "release_lock",
    "watch_document",
    "unwatch_document",
    "updates_possible",
    "check_for_update",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
