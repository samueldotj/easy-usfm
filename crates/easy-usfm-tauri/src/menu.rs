//! The native menu bar.
//!
//! PRODUCT §4 asks for a native title bar and menus. That is not decoration:
//! a translator who has used Windows for twenty years looks for New under
//! File, presses Alt+F, and expects the accelerator shown beside the item to
//! be the one that works. A row of buttons pretending to be a menu fails all
//! three.
//!
//! Built here rather than in the interface because a menu bar *is* the shell —
//! the webview cannot draw one, and drawing something that looks like one is
//! how applications end up feeling foreign on every platform at once.
//!
//! Per-platform refinement is P6.1: macOS wants the application menu first and
//! Quit inside it, Linux conventions differ again. What is here is the
//! structure and the accelerators, which is what P1.10 asked for.

use tauri::menu::{
    AboutMetadata, Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Runtime};

/// The event the interface listens for. One channel, with the item's id as the
/// payload, so adding an item does not mean adding a listener.
pub const MENU_EVENT: &str = "menu";

/// How many recent files the menu shows.
const RECENT_SHOWN: usize = 10;

/// Builds the menu bar, with `recent` filling the Open Recent submenu.
pub fn build<R: Runtime>(app: &AppHandle<R>, recent: &[String]) -> tauri::Result<Menu<R>> {
    let mut open_recent = SubmenuBuilder::new(app, "Open &Recent");

    if recent.is_empty() {
        // An empty submenu that cannot be opened is more confusing than a
        // disabled item saying why it is empty.
        open_recent = open_recent.item(
            &MenuItemBuilder::with_id("recent:none", "No recent files")
                .enabled(false)
                .build(app)?,
        );
    } else {
        for path in recent.iter().take(RECENT_SHOWN) {
            let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
            open_recent = open_recent.item(
                // The full path is the id, so choosing an item needs no lookup
                // table that could fall out of step with what is displayed.
                &MenuItemBuilder::with_id(format!("recent:{path}"), name).build(app)?,
            );
        }
        open_recent = open_recent
            .separator()
            .item(&MenuItemBuilder::with_id("recent:clear", "Clear Recent Files").build(app)?);
    }

    let file = SubmenuBuilder::new(app, "&File")
        .item(
            &MenuItemBuilder::with_id("new", "&New")
                .accelerator("CmdOrCtrl+N")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("open", "&Open…")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .item(&open_recent.build()?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("save", "&Save")
                .accelerator("CmdOrCtrl+S")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("save-as", "Save &As…")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(app)?,
        )
        .separator()
        // Predefined, so it does what the platform's own Quit does -- which
        // includes going through the window's close handler, and therefore
        // through the unsaved-changes prompt.
        .item(&PredefinedMenuItem::quit(app, Some("E&xit"))?)
        .build()?;

    // Predefined throughout. These are handled by the webview itself, so
    // routing them through our own event would mean reimplementing undo.
    let edit = SubmenuBuilder::new(app, "&Edit")
        .item(&PredefinedMenuItem::undo(app, Some("&Undo"))?)
        .item(&PredefinedMenuItem::redo(app, Some("&Redo"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some("Cu&t"))?)
        .item(&PredefinedMenuItem::copy(app, Some("&Copy"))?)
        .item(&PredefinedMenuItem::paste(app, Some("&Paste"))?)
        .separator()
        .item(&PredefinedMenuItem::select_all(app, Some("Select &All"))?)
        .build()?;

    let view = SubmenuBuilder::new(app, "&View")
        .item(&MenuItemBuilder::with_id("theme:light", "&Light Theme").build(app)?)
        .item(&MenuItemBuilder::with_id("theme:dark", "&Dark Theme").build(app)?)
        .item(&MenuItemBuilder::with_id("theme:system", "&System Theme").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("focus-editor", "&Focus Editor")
                .accelerator("F6")
                .build(app)?,
        )
        .build()?;

    let help = SubmenuBuilder::new(app, "&Help")
        .item(&PredefinedMenuItem::about(
            app,
            Some("&About Easy USFM"),
            Some(AboutMetadata {
                name: Some("Easy USFM".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some("An editor for individual USFM Scripture files.".into()),
                license: Some("MIT".into()),
                website: Some("https://github.com/samueldotj/easy-usfm".into()),
                ..Default::default()
            }),
        )?)
        .build()?;

    MenuBuilder::new(app)
        .items(&[&file, &edit, &view, &help])
        .build()
}

/// Installs the menu and forwards its events to the interface.
pub fn install<R: Runtime>(app: &AppHandle<R>, recent: &[String]) -> tauri::Result<()> {
    app.set_menu(build(app, recent)?)?;

    app.on_menu_event(|app, event| {
        // One event with the id as payload. The interface already knows how to
        // do each of these -- the menu is another way to ask, not a second
        // implementation.
        let _ = app.emit(MENU_EVENT, event.id().0.as_str());
    });

    Ok(())
}

/// Rebuilds the menu so Open Recent matches what the interface holds.
///
/// The list lives in the interface's settings, and a native submenu cannot
/// read it — so it is pushed here whenever it changes. Rebuilding the whole
/// bar rather than mutating one submenu is not the efficient choice; it is the
/// one where the menu cannot end up half-updated.
#[tauri::command]
pub fn set_recent_files<R: Runtime>(app: AppHandle<R>, paths: Vec<String>) -> Result<(), String> {
    let menu = build(&app, &paths).map_err(|error| error.to_string())?;
    app.set_menu(menu).map_err(|error| error.to_string())?;
    Ok(())
}
