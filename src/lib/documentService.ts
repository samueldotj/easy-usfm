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
  /** What this host cannot do, for the interface to say plainly. */
  readonly limitations: readonly string[];
}

/** What a service needs to know about the document to save it. */
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
