# Changelog

Notable changes, newest first. Versions follow [semantic versioning](https://semver.org/).

## Unreleased

* **A `\rem` no longer hides the chapter after it.** When a `\rem` came while
  a paragraph was open — typically the last line of an introduction, written
  by the tool that exported the file — the parser made the next `\c` or `\p`
  a child of that paragraph. Chapter 1 then did not exist as far as
  validation could tell: every paragraph in it was "not allowed before the
  first `\c` marker", chapter 2 was out of sequence, and a one-chapter book
  had no chapter at all. A Gujarati New Testament reported 214 such errors
  across 27 books, none of them real. The blocks are now moved back out and
  validated where they belong, so the tree, the verse index and the
  diagnostics are all right, and a paragraph that really does come before the
  first chapter is reported on the right line.

## 0.2.0

* **A file given on the command line is opened.** `run` never looked at its
  arguments, so the only ways into a document were the dialog and the recent
  list — a file association had nothing to hand a path to, and BibleCompose,
  which shares this engine and so reports the same diagnostics, could not take
  a publisher from a warning against a book to the file that caused it. The
  first argument naming a file that exists is now opened at startup. It is
  asked for by the interface rather than pushed at it: an event emitted while
  the shell is still setting up fires before anything is listening, and the
  file is then lost silently, which reads as "it just opened empty".

* **The confirmation dialogs ask something now.** Two faults, and the first hid
  the second. `dialog:allow-confirm` and `dialog:allow-message` were never
  granted, so the call rejected and no dialog was drawn. And `window.confirm`
  on the desktop is not the browser's — Tauri replaces it with an `async`
  function, because a WebView2 modal would block the thread the application
  runs on, so it returns a promise; a promise is an object and an object is
  truthy. Every `if (!confirm(…)) return;` guard therefore passed whatever the
  person answered, with or without the permission. The identical line is
  correct in a browser, which is why it survived: the build that is easy to
  drive from a test is the build where it works.

* **The editor builds installers.** A release workflow produces `.msi`,
  `.exe`, `.dmg`, `.deb` and `.AppImage` on a `v*` tag and drafts a release
  from them, with a `SHA256SUMS` over the lot. **They are unsigned**: Windows
  warns through SmartScreen on first run and macOS quarantines the application
  until it is opened from the context menu once. Certificates are bought
  rather than written, and a build that works unsigned is more useful than one
  that fails because a secret is missing.

## 0.1.0

The first tagged build.
