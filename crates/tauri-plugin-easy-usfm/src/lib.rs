//! The editor's desktop shell, as a Tauri plugin.
//!
//! ARCHITECTURE §2: native Rust handles only what browsers cannot — dialogs,
//! reading, atomic writing, watching, recovery. **No parsing happens here.**
//! There is one engine on every target and it runs in a worker, so this crate
//! stays a file-access layer however tempting it becomes to call the parser it
//! already links against.
//!
//! # Two hosts
//!
//! A plugin rather than an application because there are two applications.
//! easy-usfm's own window registers it and adopts its `main` window; and
//! BibleCompose registers it and calls [`open_editor`] on one of a project's
//! books, which opens an editor window in BibleCompose's own process. Either
//! way the save ladder, recovery and watching are this code, so a book edited
//! from inside BibleCompose is written exactly as easy-usfm would write it.
//!
//! Everything a window holds — the file it was opened on, what it is
//! watching, its recent list, its menu — is kept per window (see
//! [`editors`]). One process, several editors, and a file change or a menu
//! choice reaches only the window it is about.

pub mod document;
pub mod editors;
pub mod figure;
pub mod fs;
pub mod lock;
pub mod menu;
pub mod recovery;
pub mod reopen;
pub mod save;
pub mod updates;
pub mod watch;

use serde::Serialize;
use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime};

pub use editors::{editor_labels, is_editor, open_editor, EDITOR_LABEL_PREFIX};

/// What the plugin is called: the `plugin:easy-usfm|…` the interface invokes,
/// and the `easy-usfm:default` a capability grants.
pub const NAME: &str = "easy-usfm";

/// What File offers, and therefore what the window is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileMenu {
    /// easy-usfm's own: New, Open, Open Recent, and Exit.
    Full,
    /// A window opened on one book by its host. The book it was opened on is
    /// the only one it opens, and it closes itself rather than the host.
    Book,
}

/// How the plugin was configured, fixed for the life of the process.
#[derive(Debug, Clone)]
pub struct Options {
    pub file_menu: FileMenu,
    /// Inside the host's data directory. Its own name, so that a host keeps
    /// its editors' recovery apart from easy-usfm's.
    pub recovery_folder: String,
    /// Windows created by configuration rather than by [`open_editor`] that
    /// are editors all the same — easy-usfm's `main`.
    pub adopt: Vec<String>,
    /// Whether the first file named on the command line is the adopted
    /// window's to open. Only the host that *is* the editor wants this:
    /// BibleCompose's arguments are BibleCompose's.
    pub launch_from_args: bool,
}

/// What the interface asks for to learn which of the two it is in.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub file_menu: FileMenu,
}

/// Configures the plugin.
pub struct Builder {
    options: Options,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            options: Options {
                file_menu: FileMenu::Full,
                recovery_folder: "recovery".to_string(),
                adopt: Vec::new(),
                launch_from_args: false,
            },
        }
    }

    /// Editor windows open one book and close themselves (see [`FileMenu`]).
    pub fn book_only(mut self) -> Self {
        self.options.file_menu = FileMenu::Book;
        self
    }

    /// Where, inside the application's data directory, snapshots and locks
    /// are kept. `recovery` unless set.
    pub fn recovery_folder(mut self, name: impl Into<String>) -> Self {
        self.options.recovery_folder = name.into();
        self
    }

    /// Treat the window with this label as an editor.
    pub fn adopt(mut self, label: impl Into<String>) -> Self {
        self.options.adopt.push(label.into());
        self
    }

    /// Hand the first file on the command line to the first adopted window.
    pub fn launch_from_args(mut self) -> Self {
        self.options.launch_from_args = true;
        self
    }

    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        let options = self.options;
        tauri::plugin::Builder::new(NAME)
            .invoke_handler(tauri::generate_handler![
                engine_version,
                shell_profile,
                editors::launched_with,
                editors::set_recent_files,
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
            .setup(move |app, _api| {
                let launched = if options.launch_from_args {
                    editors::first_file(
                        std::env::args_os()
                            .skip(1)
                            .filter_map(|a| a.into_string().ok()),
                    )
                } else {
                    None
                };
                app.manage(editors::Editors::new(&options, launched));
                app.manage(options.clone());
                app.manage(document::Documents::default());
                // Fixed for the process's lifetime, so a pid recorded in a
                // lock can be told apart from the same number reused by
                // something else (P4.2).
                app.manage(recovery::Started(lock::now_ms()));
                app.manage(watch::FileWatch::default());

                editors::forward_menu_events(app);

                // FILE-FIDELITY 4: "directories older than 30 days pruned at
                // startup". Off the main thread -- it is a directory walk over
                // however much has accumulated, and nothing is waiting on it.
                let handle = app.clone();
                std::thread::spawn(move || {
                    if let Ok(root) = recovery::root(&handle) {
                        recovery::prune(&fs::RealFs, &root, std::time::SystemTime::now());
                    }
                });
                Ok(())
            })
            .on_window_ready(editors::window_ready)
            .build()
    }
}

/// The plugin with easy-usfm's own defaults.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}

/// The engine version, and the round trip that proves the two halves are
/// talking.
#[tauri::command]
fn engine_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Which of the two hosts this window is in, as far as the interface needs to
/// know: whether it may offer New and Open.
#[tauri::command]
fn shell_profile(options: tauri::State<'_, Options>) -> Profile {
    Profile {
        file_menu: options.file_menu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_reported() {
        assert_eq!(engine_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn the_profile_says_which_file_menu() {
        let json = serde_json::to_string(&Profile {
            file_menu: FileMenu::Book,
        })
        .expect("serializes");
        assert_eq!(json, r#"{"fileMenu":"book"}"#);
    }
}
