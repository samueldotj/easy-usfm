/**
 * The boundary between the interface and whatever is hosting it.
 *
 * ARCHITECTURE §2: Svelte components never call Tauri APIs directly. The same
 * frontend runs in the desktop shell and in a browser (M5), so anything only
 * one of them can do is asked for through here and nothing in the component
 * tree branches on platform.
 *
 * Today this is one call. The document lifecycle — open, save, save as —
 * joins it at P1.10, and the web implementation at P2.12.
 */

/** Whether the desktop shell is hosting us. */
export function isDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * What the native shell's plugin is called.
 *
 * The shell is a Tauri plugin so that two applications can carry it —
 * easy-usfm's own window, and BibleCompose, which opens this editor on one of
 * a project's books in its own process. A plugin's commands are reached as
 * `plugin:<name>|<command>`, and every call in this interface goes through
 * {@link invokeShell} so that spelling is in one place.
 */
export const SHELL_PLUGIN = "easy-usfm";

/** Calls one of the shell's own commands. */
export async function invokeShell<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(`plugin:${SHELL_PLUGIN}|${command}`, args);
}

/**
 * Listens for an event sent to *this* window.
 *
 * Not the global `listen`, which hears an event whichever window it was sent
 * to. Inside BibleCompose there may be an editor per book in one process, and
 * a file changing under Genesis is not news to the window editing Exodus.
 */
export async function listenHere<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  const { getCurrentWebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  return getCurrentWebviewWindow().listen<T>(event, (e) => handler(e.payload));
}

/**
 * Whether this window was opened by a host on one book.
 *
 * Such a window edits that book and nothing else: which file is open is the
 * host's to say, so it offers no New, no Open and no Open Recent. The shell
 * enforces the same thing on its side; this is so nothing is offered that
 * would only be refused.
 */
let bookOnly = false;

/** Asks the shell which kind of window this is. Once, at startup. */
export async function readProfile(): Promise<void> {
  if (!isDesktop()) return;
  try {
    const profile = await invokeShell<{ fileMenu: string }>("shell_profile");
    bookOnly = profile.fileMenu === "book";
  } catch {
    bookOnly = false;
  }
}

/** Whether a command id is one this window offers. */
export function offered(id: string): boolean {
  if (!bookOnly) return true;
  return id !== "new" && id !== "open" && !id.startsWith("recent:");
}

/**
 * Asks the user a yes-or-no question, and waits for the answer.
 *
 * # Why this exists rather than a bare `confirm()`
 *
 * On the desktop `window.confirm` is not the browser's. Tauri replaces it with
 *
 * ```js
 * async function (message) { return await invoke("plugin:dialog|confirm", …) }
 * ```
 *
 * because a WebView2 modal would block the thread the application runs on. An
 * `async` function returns a Promise, a Promise is an object, and an object is
 * truthy — so the shape every one of these call sites was written in,
 *
 * ```js
 * if (!confirm("are you sure?")) return;   // never true on the desktop
 * ```
 *
 * reads as "the user said yes" whatever the user says, and the dialog they are
 * answering is still on screen while the code behind it has already run. In a
 * browser the very same line is correct, which is why this survived: the one
 * build that can be driven from a test is the one build where it works.
 *
 * It had teeth. The confirmation SECURITY §2 requires before a link out of a
 * document is handed to the operating system was one of them, so a link opened
 * on a yes nobody gave.
 *
 * Returning a Promise from here makes the `await` compulsory — a caller who
 * forgets it gets the same truthy object, but now with a type error on it.
 */
export async function confirmed(question: string): Promise<boolean> {
  // Awaited, which is the entire point: in a browser this is a boolean and
  // awaiting one is a no-op, and on the desktop it is the dialog's answer.
  return await Promise.resolve(confirm(question));
}

/**
 * The engine's version, which is also the round trip that proves the shell and
 * the Rust side are talking to each other.
 */
export async function engineVersion(): Promise<string> {
  if (!isDesktop()) {
    // The browser build has no shell to ask. It will get its answer from the
    // WASM worker instead (P2.1); until then it says so rather than pretending.
    return "web (engine not yet loaded)";
  }

  return invokeShell<string>("engine_version");
}
