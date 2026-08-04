// Windows release builds open no console window. Kept off debug builds so
// panics and logging remain visible while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    easy_usfm_tauri_lib::run()
}
