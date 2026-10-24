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

  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<string>("engine_version");
}
