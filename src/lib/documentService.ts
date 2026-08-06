/**
 * The file lifecycle, as the interface asks for it.
 *
 * ARCHITECTURE §2 names this interface and says the implementations are
 * "selected at build time" so that "nothing in the component tree branches on
 * platform". The selection is made once, here, at the bottom of the tree; the
 * Tauri implementation's imports are dynamic, so a browser build never fetches
 * them and the practical effect — no shell code in the web bundle — is the
 * same as a build-time swap without a second build configuration to keep
 * correct.
 *
 * What the two implementations do *not* differ on is fidelity. The desktop
 * reads the envelope in native Rust; the web reads it with the same code
 * compiled to WASM. FILE-FIDELITY's guarantees are not a desktop feature.
 */

import type { Eol } from "./eol";

export interface Summary {
  encoding: string;
  eol: string;
  bom: boolean;
  final_newline: boolean;
  mixed_eol: boolean;
}

/** A file that has been opened. */
export interface Opened {
  /** The shell's handle on it, where there is one. */
  id: number | null;
  /** Displayable location. On the web this is a file name, not a path. */
  path: string | null;
  text: string;
  summary: Summary;
  /** One terminator per newline, for the editor to map through edits. */
  eols: Eol[];
}

/** What a save did. */
export interface SaveOutcome {
  path: string | null;
  summary: Summary;
  /** Which rung of the ladder wrote it, for the status bar. */
  note: string | null;
  /** False when the user cancelled a dialog; not an error. */
  saved: boolean;
}

/** What the interface may ask of whatever is hosting it. */
export interface DocumentService {
  /** A new, empty document. */
  createNew(): Promise<Opened>;
  /**
   * Opens a file. `path` is only meaningful on the desktop, where it comes
   * from the recent-files list.
   */
  open(path?: string): Promise<Opened | null>;
  /** Saves in place. Returns `saved: false` if there is nowhere to save to. */
  save(document: Editable): Promise<SaveOutcome>;
  saveAs(document: Editable): Promise<SaveOutcome>;
  /** Whether this host can save in place at all. */
  canSaveInPlace(document: Editable): boolean;
  /** Warn about unsaved work. */
  confirmDiscard(name: string): Promise<boolean>;
  /** Push the recent list into a native menu, where there is one. */
  setRecentFiles(paths: string[]): Promise<void>;
  /**
   * The bytes of a figure the document asked for (SECURITY §3).
   *
   * `null` where the host cannot load local files at all, which is every
   * browser: the web build has nothing to call, so "the web build never loads
   * local images" is true without a check for it.
   *
   * The path is the one the `\fig` carried, sent as written. Resolving it
   * against the document's folder happens on the other side, so this side
   * never holds a path outside what the user opened -- and a rejection comes
   * back as an error message rather than as silence.
   */
  readFigure(id: number | null, path: string): Promise<Uint8Array | null>;
  /**
   * Writes a recovery snapshot (FILE-FIDELITY §4).
   *
   * Fire and forget from the caller's side, and silent on failure: a snapshot
   * is a safety net nobody asked for, and a dialog saying it could not be
   * written would interrupt the typing it exists to protect.
   */
  snapshot(document: Editable, state: SnapshotState): Promise<void>;
  /** Forgets them, on a clean save or a clean close. */
  clearSnapshots(document: Editable): Promise<void>;
  /**
   * Who holds a file, and whether unsaved work is waiting (FILE-FIDELITY §4).
   *
   * Asked before the file is shown, because the answer decides what is shown.
   * `null` where the host cannot answer — a browser has no processes to ask
   * about and no snapshots yet (P4.6).
   */
  examine(path: string): Promise<Reopen | null>;
  /** Records this process as the holder. */
  takeLock(path: string): Promise<void>;
  /** Gives it up, on a clean close. */
  releaseLock(path: string): Promise<void>;
  /**
   * Watches a file for changes made outside this window (FILE-FIDELITY §3).
   *
   * The handler is called only for changes that are genuinely someone else's —
   * our own saves are recognised by content hash and never reported.
   */
  watch(path: string, onchange: (change: FileChanged) => void): Promise<void>;
  unwatch(): Promise<void>;
  /** What this host cannot do, for the interface to say plainly. */
  readonly limitations: readonly string[];
}

/** What a service needs to know about the document to save it. */
/** A change to the file made outside this window (FILE-FIDELITY §3). */
export interface FileChanged {
  /** `"external"` — someone edited it. `"gone"` — it was deleted or renamed. */
  kind: "external" | "gone";
  path: string;
  /** What it now holds. Absent when it is gone. */
  text: string | null;
}

/** Who has a file open, if anyone (FILE-FIDELITY §4). */
export type Held =
  | { state: "free" }
  | { state: "ours" }
  | { state: "foreign"; owner: Owner }
  | { state: "crashed"; owner: Owner };

export interface Owner {
  pid: number;
  started_at: number;
  host: string;
  app_version: string;
}

/** Unsaved work from a session that did not finish. */
export interface Recovery {
  taken_at: number;
  lines_differing: number;
  text: string;
  cursor: number;
}

export interface Reopen {
  held: Held;
  recovery: Recovery | null;
}

/**
 * What a snapshot records beyond the text (FILE-FIDELITY §4).
 *
 * Passed alongside {@link Editable} rather than added to it: `Editable` is what
 * *saving* needs, and a caret position is not part of that. Widening it would
 * make every caller of `save` carry a cursor it has no use for.
 */
export interface SnapshotState {
  /** UTF-16 code units, the unit the editor selects in. */
  cursor: number;
  dirty: boolean;
  /**
   * The dominant terminator, in the spelling the engine uses — not the one the
   * status bar shows. `Eol` is what a recovery has to restore the envelope
   * from, and `"LF"` is a label for people.
   */
  eol: Eol;
  finalNewline: boolean;
}

export interface Editable {
  id: number | null;
  path: string | null;
  text: string;
  eols: Eol[];
  bom: boolean;
}

let service: DocumentService | null = null;

/**
 * The service for this host, chosen once.
 *
 * Cached rather than re-derived, because both implementations hold state — the
 * desktop a document id, the web a file handle — and a second instance would
 * silently lose whichever one the interface was not holding.
 */
export async function documentService(): Promise<DocumentService> {
  if (service) return service;

  const desktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  service = desktop
    ? (await import("./documentService.tauri")).tauriDocuments()
    : (await import("./documentService.web")).webDocuments();

  return service;
}
