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
//! # Per platform (P6.1)
//!
//! Two kinds of difference, and they are not the same kind of thing.
//!
//! **Structure.** macOS puts an application menu first, with About, Services,
//! Hide and Quit inside it, and nothing else may precede it. Windows and Linux
//! put Exit at the foot of File and About under Help. This is not a preference:
//! a macOS user looking for Quit looks in exactly one place, and an application
//! that puts it under File is visibly not a Mac application.
//!
//! **Accelerators.** PRODUCT 6.4 gives a table with a column per platform, and
//! most rows agree because `CmdOrCtrl` already covers them. Four do not, and
//! each disagrees for a reason: on macOS, Cmd+G is Find Next by universal
//! convention, which pushes Go to Reference to Cmd+L (as Xcode and VS Code do)
//! and Replace to Option+Cmd+F. Following the table rather than translating
//! Ctrl to Cmd is the whole point -- a mechanical translation produces exactly
//! the bindings that fight the platform.
//!
//! Chosen with `cfg!` rather than at build time by feature, so both arms are
//! type-checked on every platform and a rename cannot break the one nobody is
//! compiling today.

use tauri::menu::{
    AboutMetadata, Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Runtime};

/// The event the interface listens for. One channel, with the item's id as the
/// payload, so adding an item does not mean adding a listener.
pub const MENU_EVENT: &str = "menu";

/// How many recent files the menu shows.
const RECENT_SHOWN: usize = 10;

/// Whether this build is for macOS, in a form both arms compile on.
const MAC: bool = cfg!(target_os = "macos");

/// One row of PRODUCT §6.4, for the four actions whose binding differs.
///
/// A function rather than a `cfg` block per item, so the two columns of the
/// table sit next to each other and can be read against it.
const fn shortcut(windows_linux: &'static str, macos: &'static str) -> &'static str {
    if MAC {
        macos
    } else {
        windows_linux
    }
}

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

    // Insert. The same commands the toolbar offers, by the same ids -- the
    // interface has one handler for both, so a menu item cannot come to mean
    // something different from the button beside it.
    //
    // Bold and Italic take the accelerators every editor uses. The rest have
    // none: there are seven of them, the letters worth having are taken, and a
    // shortcut nobody can guess is a line in a menu rather than a shortcut.
    let insert = SubmenuBuilder::new(app, "&Insert")
        .item(
            &MenuItemBuilder::with_id("insert-chapter", "&Chapter")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("insert-verse", "&Verse").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("insert-bold", "&Bold")
                .accelerator("CmdOrCtrl+B")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("insert-italic", "&Italic")
                .accelerator("CmdOrCtrl+I")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("insert-paragraph", "&Paragraph").build(app)?)
        .item(&MenuItemBuilder::with_id("insert-break", "Blan&k Line").build(app)?)
        .item(&MenuItemBuilder::with_id("insert-poetry", "Poetr&y Line").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("insert-table", "&Table").build(app)?)
        .item(&MenuItemBuilder::with_id("insert-figure", "I&mage").build(app)?)
        .build()?;

    let mut file = SubmenuBuilder::new(app, "&File")
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
        .item(
            &MenuItemBuilder::with_id("print", "&Print…")
                .accelerator("CmdOrCtrl+P")
                .build(app)?,
        );

    // Quit lives in the application menu on macOS, and nowhere else may claim
    // it: a Mac user looking for it looks in exactly one place. Everywhere else
    // it is Exit at the foot of File. Predefined either way, so it does what
    // the platform's own Quit does -- which includes going through the
    // window's close handler and therefore the unsaved-changes prompt.
    if !MAC {
        file = file
            .separator()
            .item(&PredefinedMenuItem::quit(app, Some("E&xit"))?);
    }
    let file = file.build()?;

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
        .separator()
        // Find is ours rather than predefined: the search runs against the
        // engine's normalized index, so the webview's own find would answer a
        // different question (UNICODE §4).
        .item(
            &MenuItemBuilder::with_id("find", "&Find…")
                .accelerator("CmdOrCtrl+F")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("replace", "&Replace…")
                // Ctrl+H is Delete Backwards on macOS, from emacs by way of
                // the text system, so it is not available for Replace there.
                .accelerator(shortcut("CmdOrCtrl+H", "Alt+Cmd+F"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("find-next", "Find &Next")
                .accelerator(shortcut("F3", "Cmd+G"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("find-previous", "Find &Previous")
                .accelerator(shortcut("Shift+F3", "Shift+Cmd+G"))
                .build(app)?,
        )
        .build()?;

    let view = SubmenuBuilder::new(app, "&View")
        .item(&MenuItemBuilder::with_id("theme:light", "&Light Theme").build(app)?)
        .item(&MenuItemBuilder::with_id("theme:dark", "&Dark Theme").build(app)?)
        .item(&MenuItemBuilder::with_id("theme:system", "&System Theme").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("focus-editor", "&Focus Editor")
                .accelerator("CmdOrCtrl+1")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("focus-preview", "Focus Pre&view")
                .accelerator("CmdOrCtrl+2")
                .build(app)?,
        )
        .item(
            // F6 cycles rather than choosing, which is the platform convention
            // for moving between panes and is why it is a separate item from
            // the two above (PRODUCT 6.4).
            &MenuItemBuilder::with_id("cycle-pane", "&Cycle Pane Focus")
                .accelerator("F6")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("toggle-invisibles", "Show &Invisible Characters")
                .accelerator("CmdOrCtrl+Shift+8")
                .build(app)?,
        )
        // Per document and off every time one opens (SECURITY 3), so this is
        // a view toggle rather than a setting -- which is why it sits here
        // beside the other one and not in a preferences dialog.
        .item(&MenuItemBuilder::with_id("toggle-images", "Show I&mages").build(app)?)
        .separator()
        // Cmd+G is Find Next on macOS by universal convention, so Go to
        // Reference takes Cmd+L there -- which is what Xcode and VS Code do.
        .item(
            &MenuItemBuilder::with_id("go-to-reference", "&Go to Reference…")
                .accelerator(shortcut("CmdOrCtrl+G", "Cmd+L"))
                .build(app)?,
        )
        .separator()
        // The accelerators are declared here and only here. The interface's own
        // key handler stands down on the desktop, so a shortcut shown beside a
        // menu item is the one that runs -- and there is no second binding to
        // fall out of step with what the menu claims (PRODUCT §6.4).
        .item(
            &MenuItemBuilder::with_id("toggle-diagnostics", "&Diagnostics")
                .accelerator("CmdOrCtrl+Shift+M")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("next-diagnostic", "&Next Diagnostic")
                .accelerator("F8")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("previous-diagnostic", "&Previous Diagnostic")
                .accelerator("Shift+F8")
                .build(app)?,
        )
        .build()?;

    // The About box, which appears in a different menu on each platform but is
    // the same item. Built once so the metadata cannot drift between them.
    let about = PredefinedMenuItem::about(
        app,
        Some(if MAC { "About Easy USFM" } else { "&About Easy USFM" }),
        Some(AboutMetadata {
            name: Some("Easy USFM".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            comments: Some("An editor for individual USFM Scripture files.".into()),
            license: Some("MIT".into()),
            website: Some("https://github.com/samueldotj/easy-usfm".into()),
            ..Default::default()
        }),
    )?;

    let help = SubmenuBuilder::new(app, "&Help");
    // On macOS About belongs in the application menu and Help holds only
    // documentation. Putting it in both would be two About items.
    let help = if MAC { help } else { help.item(&about) };
    let help = help.build()?;

    if !MAC {
        return MenuBuilder::new(app)
            .items(&[&file, &edit, &insert, &view, &help])
            .build();
    }

    // The application menu: first, named for the application, and holding the
    // items macOS puts nowhere else. Services, Hide, Hide Others and Show All
    // are predefined because they are the system's, not ours -- an application
    // that omits them is one whose Cmd+H does nothing.
    let application = SubmenuBuilder::new(app, "Easy USFM")
        .item(&about)
        .separator()
        .item(&PredefinedMenuItem::services(app, Some("Services"))?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("Hide Easy USFM"))?)
        .item(&PredefinedMenuItem::hide_others(app, Some("Hide Others"))?)
        .item(&PredefinedMenuItem::show_all(app, Some("Show All"))?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Quit Easy USFM"))?)
        .build()?;

    // Window is a macOS convention with no equivalent elsewhere: Minimise and
    // Zoom live there, and the system adds the window list to it itself.
    let window = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, Some("Minimize"))?)
        .item(&PredefinedMenuItem::maximize(app, Some("Zoom"))?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, Some("Close Window"))?)
        .build()?;

    MenuBuilder::new(app)
        .items(&[&application, &file, &edit, &insert, &view, &window, &help])
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
