/**
 * What the main thread and the engine worker say to each other.
 *
 * ARCHITECTURE §8.3: every message carries a monotonic `rev`, the main thread
 * holds `latestAppliedRev`, and lower-revision results are discarded. Stale
 * results never replace newer state — which matters because the worker answers
 * out of order under load, and a late reply carrying old offsets would move the
 * cursor and light up diagnostics on text that has since been retyped.
 *
 * Declared in one file both sides import, so a change to the protocol is a
 * type error rather than a runtime surprise.
 */

/** A diagnostic, in UTF-16 code units (UNICODE §1). */
export interface Diagnostic {
  code: string;
  severity: "error" | "warning" | "information";
  start: number;
  end: number;
  message: string;
}

/** One chapter's worth of document. The unit the preview keys on. */
export interface Chunk {
  number: number | null;
  start: number;
  end: number;
  rev: number;
}

export interface ParseResult {
  rev: number;
  chunks: Chunk[];
  diagnostics: Diagnostic[];
  /** UTF-16 length, for checking the mirror against this side. */
  len: number;
}

/** One edit, in the coordinates CodeMirror reports. */
export interface Edit {
  from: number;
  to: number;
  insert: string;
}

export type Request =
  | { kind: "open"; rev: number; text: string }
  | { kind: "edit"; rev: number; edits: Edit[] }
  /** The full document again, after a desync or an external reload. */
  | { kind: "resync"; rev: number; text: string }
  | { kind: "version"; rev: number };

export type Response =
  | { kind: "ready" }
  | { kind: "parsed"; rev: number; result: ParseResult }
  | { kind: "version"; rev: number; version: string }
  /**
   * The worker could not apply what it was sent and its mirror is no longer
   * trustworthy. The main thread must resend the whole document rather than
   * carry on — ARCHITECTURE §9: silent drift corrupts every offset in the
   * interface.
   */
  | { kind: "desync"; rev: number; reason: string };
