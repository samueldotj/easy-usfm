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
