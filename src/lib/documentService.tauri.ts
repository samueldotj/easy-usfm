/**
 * The desktop half of {@link DocumentService}.
 *
 * Every Tauri import here is dynamic, so a browser build never fetches this
 * module's dependencies — the bundler splits them out and nothing loads them.
 *
 * All the real work is in `easy-usfm-tauri`: the save ladder, the fidelity
 * envelope, the recent-files menu. This is the wire between that and the
 * interface, and it deliberately holds no logic of its own — a rule that
 * matters because the web implementation *does* hold logic, and any behaviour
 * that lived here would be behaviour the two shells disagree on.
 */

import type {
  DocumentService,
  Editable,
  Opened,
  SaveOutcome,
  FileChanged,
  Reopen,
  SnapshotState,
  Summary,
} from "./documentService";
import type { Eol } from "./eol";

interface NativeOpened {
  id: number;
  path: string | null;
  text: string;
  summary: Summary;
  eols: Eol[];
}

interface NativeSave {
  path: string;
  summary: Summary;
  note: string | null;
}

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: call } = await import("@tauri-apps/api/core");
  return call<T>(command, args);
}

export function tauriDocuments(): DocumentService {
  /**
   * The event listener, attached once and kept.
   *
   * Tauri's `listen` returns an unlisten function and attaching a second one
   * would deliver every change twice. The shell watches one file at a time, so
   * re-pointing it is a command rather than a new subscription.
   */
  let unlisten: (() => void) | null = null;
  let handler: ((change: FileChanged) => void) | null = null;

  return {
    limitations: [],

    async createNew(): Promise<Opened> {
      return invoke<NativeOpened>("new_document");
    },

    async open(path?: string): Promise<Opened | null> {
      let chosen = path;
      if (!chosen) {
        const { open } = await import("@tauri-apps/plugin-dialog");
        const picked = await open({
          multiple: false,
          directory: false,
          filters: [{ name: "USFM", extensions: ["usfm", "sfm", "SFM", "USFM"] }],
        });
        if (typeof picked !== "string") return null;
        chosen = picked;
      }
      return invoke<NativeOpened>("open_document", { path: chosen });
    },

    canSaveInPlace(document: Editable): boolean {
      return document.id !== null && document.path !== null;
    },

    /**
     * Saves in place.
     *
     * A read-only target is not an error. FILE-FIDELITY §2's third rung is to
     * refuse and offer Save As, and that is what the message means — so it is
     * turned back into a Save As here rather than surfacing as a failure the
     * user has to interpret.
     */
    async save(document: Editable): Promise<SaveOutcome> {
      if (document.id === null) return notSaved();
      if (!document.path) return this.saveAs(document);

      try {
        const report = await invoke<NativeSave>("save_document", {
          request: { id: document.id, text: document.text, eols: document.eols },
          path: null,
        });
        return { ...report, saved: true };
      } catch (error) {
        if (String(error).includes("READONLY:")) return this.saveAs(document);
        throw error;
      }
    },

    async saveAs(document: Editable): Promise<SaveOutcome> {
      if (document.id === null) return notSaved();

      const { save } = await import("@tauri-apps/plugin-dialog");
      const chosen = await save({
        defaultPath: document.path ?? "untitled.usfm",
        filters: [{ name: "USFM", extensions: ["usfm", "sfm"] }],
      });
      if (typeof chosen !== "string") return notSaved();

      const report = await invoke<NativeSave>("save_document", {
        request: { id: document.id, text: document.text, eols: document.eols },
        path: chosen,
      });
      return { ...report, saved: true };
    },

    async confirmDiscard(name: string): Promise<boolean> {
      const { ask } = await import("@tauri-apps/plugin-dialog");
      return ask(`${name} has unsaved changes. Discard them?`, {
        title: "Unsaved changes",
        kind: "warning",
        okLabel: "Discard",
        cancelLabel: "Cancel",
      });
    },

    async setRecentFiles(paths: string[]): Promise<void> {
      try {
        await invoke("set_recent_files", { paths });
      } catch {
        // A menu that failed to rebuild is not worth interrupting anyone over;
        // every command in it is still reachable by its accelerator.
      }
    },

    async readFigure(id: number | null, path: string): Promise<Uint8Array | null> {
      // No open document means no folder to be relative to, and the shell
      // would refuse anyway -- asking would only turn that into an error the
      // interface has to phrase.
      if (id === null) return null;

      // Errors propagate. SECURITY §3's refusals are the interesting outcome
      // here, not an edge case: a document asking for `../../etc/passwd` should
      // say so in the placeholder rather than look like a missing file.
      const bytes = await invoke<number[] | Uint8Array>("read_figure", { id, path });
      return bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    },

    async snapshot(document: Editable, state: SnapshotState): Promise<void> {
      // A document with no shell handle has never been through `new` or
      // `open`, so there is nothing to file a snapshot under.
      if (document.id === null) return;

      // The shell fills in the pid, version, and timestamp: a snapshot that
      // claimed to come from a process which never wrote it would make crash
      // detection (P4.2) read a crash where there was none.
      await invoke("snapshot_document", {
        id: document.id,
        path: document.path,
        text: document.text,
        meta: {
          path: document.path,
          bom: document.bom,
          eol: state.eol,
          final_newline: state.finalNewline,
          cursor: state.cursor,
          dirty: state.dirty,
          pid: 0,
          app_version: "",
          taken_at: 0,
        },
      });
    },

    async clearSnapshots(document: Editable): Promise<void> {
      if (document.id === null) return;
      await invoke("clear_recovery", { id: document.id, path: document.path });
    },

    async examine(path: string): Promise<Reopen> {
      return invoke<Reopen>("examine_document", { path });
    },

    async takeLock(path: string): Promise<void> {
      await invoke("take_lock", { path });
    },

    async releaseLock(path: string): Promise<void> {
      await invoke("release_lock", { path });
    },

    async watch(path: string, onchange: (change: FileChanged) => void): Promise<void> {
      const { listen } = await import("@tauri-apps/api/event");
      // One listener for the lifetime of the window rather than one per file:
      // the shell watches a single path at a time and re-pointing it is a
      // command, so a second listener here would double every report.
      unlisten ??= await listen<FileChanged>("file-changed", (event) => {
        handler?.(event.payload);
      });
      handler = onchange;
      await invoke("watch_document", { path });
    },

    async unwatch(): Promise<void> {
      handler = null;
      await invoke("unwatch_document");
    },
  };
}

function notSaved(): SaveOutcome {
  return {
    path: null,
    summary: { encoding: "UTF-8", eol: "lf", bom: false, final_newline: true, mixed_eol: false },
    note: null,
    saved: false,
  };
}
