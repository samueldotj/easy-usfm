//! Editor windows: which windows are editors, and what each one holds.
//!
//! easy-usfm used to be a process with one window, and its state said so — the
//! file it was launched on came from the command line, the watcher was the
//! process's, the menu was the application's and its events went to every
//! window there was. Inside BibleCompose none of that holds: one process,
//! one window per book, and a host window that is not an editor at all.
//!
//! So everything that belongs to a window is kept here under its label, and
//! everything sent to a window is sent to that window by label.
//!
//! # Menus
//!
//! Windows and Linux put a menu bar on each window, so each editor gets its
//! own and its events are its own. macOS has one menu bar for the application,
//! so the editor's menu is put up when an editor window comes forward and the
//! host's default is put back when a window that is not an editor does, and a
//! menu choice goes to whichever editor was last in front.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{
    AppHandle, Emitter, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Window,
    WindowEvent,
};

use crate::menu::{self, MENU_EVENT};
use crate::{FileMenu, Options};

/// What every window [`open_editor`] opens is labelled with, followed by a
/// hash of the file's path.
pub const EDITOR_LABEL_PREFIX: &str = "editor-";

/// Whether this build is for macOS, in a form both arms compile on.
const MAC: bool = cfg!(target_os = "macos");

#[derive(Debug, Default)]
struct Editor {
    /// The file the window was opened on, for the interface to ask for.
    launched: Option<PathBuf>,
    /// What Open Recent shows, as the interface last pushed it.
    recent: Vec<String>,
}

#[derive(Debug, Default)]
struct Inner {
    windows: HashMap<String, Editor>,
    /// The editor last in front, which is the one a macOS menu choice is for.
    focused: Option<String>,
}

/// Every editor window, by label.
#[derive(Debug, Default)]
pub struct Editors(Mutex<Inner>);

impl Editors {
    pub(crate) fn new(options: &Options, launched: Option<PathBuf>) -> Self {
        let mut inner = Inner::default();
        let mut launched = launched;
        for label in &options.adopt {
            inner.windows.insert(
                label.clone(),
                Editor {
                    // The first adopted window is the one a double-clicked
                    // file was meant for.
                    launched: launched.take(),
                    recent: Vec::new(),
                },
            );
        }
        Editors(Mutex::new(inner))
    }

    fn with<T>(&self, f: impl FnOnce(&mut Inner) -> T) -> Option<T> {
        self.0.lock().ok().map(|mut inner| f(&mut inner))
    }

    fn contains(&self, label: &str) -> bool {
        self.with(|inner| inner.windows.contains_key(label))
            .unwrap_or(false)
    }

    fn launched(&self, label: &str) -> Option<PathBuf> {
        self.with(|inner| inner.windows.get(label).and_then(|e| e.launched.clone()))
            .flatten()
    }

    fn recent(&self, label: &str) -> Vec<String> {
        self.with(|inner| {
            inner
                .windows
                .get(label)
                .map(|e| e.recent.clone())
                .unwrap_or_default()
        })
        .unwrap_or_default()
    }

    /// Whether a window opened on one book may open this file.
    ///
    /// Only the book it was opened on. Compared canonically, so the same file
    /// spelled two ways is the same file; a window with no book of its own
    /// may open nothing at all.
    pub(crate) fn may_open(&self, label: &str, path: &Path) -> bool {
        match self.launched(label) {
            Some(launched) => same_file(&launched, path),
            None => false,
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// The label the editor for this file has, whether or not it is open.
///
/// From the file, so that asking for a book that already has a window finds
/// that window rather than opening a second editor on the same bytes — the
/// second would see the first's lock and open read-only, which is a strange
/// thing to be shown for a book one has just asked to edit.
fn label_for(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let hash = blake3::hash(canonical.to_string_lossy().as_bytes());
    let hex: String = hash.as_bytes()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{EDITOR_LABEL_PREFIX}{hex}")
}

/// Opens an editor window on one file, or brings forward the one already open
/// on it.
///
/// `url` is where the host serves the editor's interface from — BibleCompose
/// stages it under `editor/`. The window asks for its file once it is ready to
/// receive it (see [`launched_with`]), so nothing is lost to an event sent
/// before anything was listening.
pub fn open_editor<R: Runtime>(
    app: &AppHandle<R>,
    path: impl Into<PathBuf>,
    url: WebviewUrl,
) -> tauri::Result<WebviewWindow<R>> {
    let path = path.into();
    let label = label_for(&path);

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        let _ = window.show();
        window.set_focus()?;
        return Ok(window);
    }

    let editors = app.state::<Editors>();
    editors.with(|inner| {
        inner.windows.insert(
            label.clone(),
            Editor {
                launched: Some(path.clone()),
                recent: Vec::new(),
            },
        )
    });

    let title = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    let built = WebviewWindowBuilder::new(app, &label, url)
        .title(title)
        .inner_size(1100.0, 750.0)
        .min_inner_size(600.0, 400.0)
        .resizable(true)
        .build();

    if built.is_err() {
        editors.with(|inner| inner.windows.remove(&label));
    }
    built
}

/// Whether the window with this label is an editor.
pub fn is_editor<R: Runtime>(app: &AppHandle<R>, label: &str) -> bool {
    app.try_state::<Editors>()
        .is_some_and(|editors| editors.contains(label))
}

/// Every editor window open now, so a host closing down can ask each to close.
pub fn editor_labels<R: Runtime>(app: &AppHandle<R>) -> Vec<String> {
    let Some(editors) = app.try_state::<Editors>() else {
        return Vec::new();
    };
    let mut labels = editors
        .with(|inner| inner.windows.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    labels.sort();
    labels
        .into_iter()
        .filter(|label| app.get_webview_window(label).is_some())
        .collect()
}

/// Puts the menu up for a window, from what it holds now.
fn install_menu<R: Runtime>(window: &Window<R>) -> tauri::Result<()> {
    let app = window.app_handle();
    let file = app.state::<Options>().file_menu;
    let recent = app.state::<Editors>().recent(window.label());
    let built = menu::build(app, &recent, file)?;
    if MAC {
        app.set_menu(built)?;
    } else {
        window.set_menu(built)?;
    }
    Ok(())
}

/// Puts the menu up from another thread, once the caller has let go.
///
/// **Never on the main thread.** `Window::set_menu` called from the main
/// thread waits on the main thread, and two places that want a menu run
/// there: Tauri's `on_window_ready` hook (which also holds its plugin store),
/// and every synchronous command, `set_recent_files` among them. Either way
/// the window opened, drew its first frame, and then never answered another
/// command — the file it was launched on was never opened. From a thread of
/// its own the menu is handed to the main thread and put up as soon as that
/// thread is free. (`AppHandle::set_menu`, which the single-window shell
/// used, does not wait like this, which is why it never showed.)
fn install_menu_soon<R: Runtime>(window: &Window<R>) {
    let window = window.clone();
    std::thread::spawn(move || {
        let _ = install_menu(&window);
    });
}

/// Called by the plugin as every window is created, editor or not.
pub(crate) fn window_ready<R: Runtime>(window: Window<R>) {
    let app = window.app_handle().clone();
    let label = window.label().to_string();
    let editor = is_editor(&app, &label);

    if editor {
        install_menu_soon(&window);
        if !MAC {
            // Its own bar, so its own events: the item's id, to this window
            // and no other.
            window.on_menu_event(|window, event| {
                let _ = window.emit_to(window.label(), MENU_EVENT, event.id().0.as_str());
            });
        }
    }

    let handle = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(true) => {
            let editors = app.state::<Editors>();
            editors.with(|inner| inner.focused = editor.then(|| label.clone()));
            if MAC {
                if editor {
                    install_menu_soon(&handle);
                } else {
                    // The host's own menu, back when the host is in front.
                    let app = app.clone();
                    std::thread::spawn(move || {
                        if let Ok(default) = tauri::menu::Menu::default(&app) {
                            let _ = app.set_menu(default);
                        }
                    });
                }
            }
        }
        WindowEvent::Destroyed if editor => {
            let editors = app.state::<Editors>();
            editors.with(|inner| {
                inner.windows.remove(&label);
                if inner.focused.as_deref() == Some(label.as_str()) {
                    inner.focused = None;
                }
            });
            app.state::<crate::watch::FileWatch>().forget(&label);
        }
        _ => {}
    });
}

/// On macOS, where the menu is the application's, a choice goes to the
/// editor last in front. Elsewhere each window forwards its own (see
/// [`window_ready`]) and this does nothing.
pub(crate) fn forward_menu_events<R: Runtime>(app: &AppHandle<R>) {
    if !MAC {
        return;
    }
    app.on_menu_event(|app, event| {
        let target = app
            .state::<Editors>()
            .with(|inner| inner.focused.clone())
            .flatten();
        if let Some(label) = target {
            let _ = app.emit_to(label.as_str(), MENU_EVENT, event.id().0.as_str());
        }
    });
}

/// The first argument that names a file which exists.
///
/// Not `args[1]`. A webview adds arguments of its own on some platforms, a
/// flag is never a path, and a publisher may open a file whose name this has
/// no opinion about. So the test is that the thing exists and is a file —
/// which also means a path typed wrongly opens an empty editor rather than an
/// error nobody asked for.
pub(crate) fn first_file<I>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = String>,
{
    args.into_iter()
        .filter(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

// -------------------------------------------------------------- commands ---

/// The file this window was opened on, for the interface to open.
///
/// **The interface asks for it rather than being told.** An event sent while
/// the window is being created fires before the interface is listening, and
/// the file is then simply lost — so it is held until the window asks.
#[tauri::command]
pub fn launched_with<R: Runtime>(
    window: Window<R>,
    editors: tauri::State<'_, Editors>,
) -> Option<String> {
    editors
        .launched(window.label())
        .and_then(|p| p.to_str().map(str::to_owned))
}

/// Rebuilds this window's menu so Open Recent matches what the interface
/// holds.
///
/// The list lives in the interface's settings, and a native submenu cannot
/// read it — so it is pushed here whenever it changes. Rebuilding the whole
/// bar rather than mutating one submenu is not the efficient choice; it is the
/// one where the menu cannot end up half-updated. A window opened on one book
/// shows no Open Recent, and the list is kept for nothing.
#[tauri::command]
pub fn set_recent_files<R: Runtime>(
    window: Window<R>,
    editors: tauri::State<'_, Editors>,
    options: tauri::State<'_, Options>,
    paths: Vec<String>,
) -> Result<(), String> {
    editors.with(|inner| {
        if let Some(editor) = inner.windows.get_mut(window.label()) {
            editor.recent = paths;
        }
    });
    if options.file_menu == FileMenu::Book {
        return Ok(());
    }
    install_menu_soon(&window);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(adopt: &[&str]) -> Options {
        Options {
            file_menu: FileMenu::Book,
            recovery_folder: "recovery".into(),
            adopt: adopt.iter().map(|s| s.to_string()).collect(),
            launch_from_args: false,
        }
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
        let as_str = |p: &Path| p.to_str().expect("UTF-8").to_owned();

        assert_eq!(first_file([as_str(&book)]), Some(book.clone()));
        assert_eq!(
            first_file(["--webview-flag".to_owned(), as_str(&book)]),
            Some(book.clone())
        );
        assert_eq!(
            first_file([as_str(&dir.path().join("nope.usfm")), as_str(&other)]),
            Some(other)
        );
        assert_eq!(first_file([as_str(dir.path())]), None);
        assert_eq!(first_file([] as [String; 0]), None);
    }

    /// The launch file belongs to the first adopted window, and only to it.
    #[test]
    fn the_launch_file_goes_to_the_first_adopted_window() {
        let editors = Editors::new(&options(&["main", "second"]), Some("a.usfm".into()));
        assert_eq!(editors.launched("main"), Some(PathBuf::from("a.usfm")));
        assert_eq!(editors.launched("second"), None);
        assert!(editors.contains("second"));
        assert!(!editors.contains("host"));
    }

    /// **Each window keeps its own.** Two editors in one process, and what one
    /// was opened on or remembers is not the other's.
    #[test]
    fn windows_do_not_share_what_they_hold() {
        let editors = Editors::new(&options(&[]), None);
        editors.with(|inner| {
            inner.windows.insert(
                "editor-a".into(),
                Editor {
                    launched: Some("a.usfm".into()),
                    recent: vec!["x".into()],
                },
            );
            inner.windows.insert(
                "editor-b".into(),
                Editor {
                    launched: Some("b.usfm".into()),
                    recent: Vec::new(),
                },
            );
        });
        assert_eq!(editors.launched("editor-a"), Some(PathBuf::from("a.usfm")));
        assert_eq!(editors.launched("editor-b"), Some(PathBuf::from("b.usfm")));
        assert_eq!(editors.recent("editor-a"), vec!["x".to_string()]);
        assert!(editors.recent("editor-b").is_empty());
    }

    /// **A window opened on a book opens that book and nothing else**, however
    /// the path is spelled.
    #[test]
    fn a_book_window_opens_only_its_book() {
        let dir = tempfile::tempdir().expect("temp dir");
        let book = dir.path().join("01_GEN.usfm");
        let other = dir.path().join("02_EXO.usfm");
        std::fs::write(&book, "\\id GEN\n").expect("write");
        std::fs::write(&other, "\\id EXO\n").expect("write");

        let editors = Editors::new(&options(&[]), None);
        editors.with(|inner| {
            inner.windows.insert(
                "editor-a".into(),
                Editor {
                    launched: Some(book.clone()),
                    recent: Vec::new(),
                },
            )
        });

        assert!(editors.may_open("editor-a", &book));
        assert!(editors.may_open("editor-a", &dir.path().join(".").join("01_GEN.usfm")));
        assert!(!editors.may_open("editor-a", &other));
        assert!(!editors.may_open("not-an-editor", &book));
    }

    /// The same file is the same window, and two files are two.
    #[test]
    fn a_label_is_the_file_it_is_for() {
        let dir = tempfile::tempdir().expect("temp dir");
        let book = dir.path().join("01_GEN.usfm");
        let other = dir.path().join("02_EXO.usfm");
        std::fs::write(&book, "").expect("write");
        std::fs::write(&other, "").expect("write");

        let label = label_for(&book);
        assert!(label.starts_with(EDITOR_LABEL_PREFIX), "{label}");
        assert_eq!(label, label_for(&dir.path().join(".").join("01_GEN.usfm")));
        assert_ne!(label, label_for(&other));
        // Tauri allows only alphanumerics, `-`, `/`, `:` and `_` in a label.
        assert!(label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }
}
