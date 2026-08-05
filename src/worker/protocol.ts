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
  /** 1-based. Computed by the engine, whose mapper is line-indexed already. */
  line: number;
  message: string;
}

/** One chapter's worth of document. The unit the preview keys on. */
export interface Chunk {
  number: number | null;
  start: number;
  end: number;
  rev: number;
}

/**
 * The document's USFM version.
 *
 * `declared` is separate from `effective` because "says nothing" is not the
 * same as "says 3.0" — most files in circulation carry no `\usfm` line and are
 * valid (PRODUCT §4), so a status bar reporting only the effective version
 * would claim a declaration the file never made.
 */
export interface UsfmVersion {
  declared: string | null;
  effective: string;
  overridden: boolean;
  /** What a file declaring nothing is taken to be. Sent, not assumed here. */
  assumed: string;
}

export interface ParseResult {
  rev: number;
  chunks: Chunk[];
  diagnostics: Diagnostic[];
  version: UsfmVersion;
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
  /**
   * A batch. `checksum` rides along every 50 batches and at each idle
   * boundary; when present the worker compares it against its own mirror and
   * reports a desync rather than carrying on.
   */
  | { kind: "edit"; rev: number; edits: Edit[]; checksum?: number }
  /** The full document again, after a desync or an external reload. */
  | { kind: "resync"; rev: number; text: string }
  /** Highlighting for a viewport, in UTF-16 offsets. */
  | { kind: "tokens"; rev: number; from: number; to: number }
  /**
   * Judge this document as the named USFM version, or `null` to go back to
   * what the file says.
   *
   * Held on the main thread and re-sent after every open, so the engine never
   * has to remember it across a resync — which it could not do anyway, since a
   * desync frees the session.
   */
  | { kind: "override-version"; rev: number; version: string | null }
  /** Go to Reference. The text exactly as typed; the engine does the parsing. */
  | { kind: "resolve"; rev: number; text: string }
  /** What reference a cursor position is at, for the status bar. */
  | { kind: "where"; rev: number; at: number }
  /** The *engine's* version, which is a different question entirely. */
  | { kind: "version"; rev: number };

/**
 * What came of looking up a reference.
 *
 * The failure carries a sentence, not a code. Every way a reference can fail
 * means something different to the person who typed it, and the engine is the
 * only side that knows which happened -- most importantly that the verse is in
 * a different file, which "not found" would never suggest.
 */
export interface Resolution {
  start: number | null;
  end: number | null;
  message: string | null;
}

/** One highlighted run. Carries a class, never a colour. */
export interface Token {
  class: string;
  start: number;
  end: number;
}

export type Response =
  | { kind: "ready" }
  | { kind: "parsed"; rev: number; result: ParseResult }
  /** Highlighting for the range that was asked for, and only that range. */
  | { kind: "tokens"; rev: number; from: number; to: number; tokens: Token[] }
  | { kind: "resolved"; rev: number; result: Resolution }
  | { kind: "where"; rev: number; reference: string | null }
  | { kind: "version"; rev: number; version: string }
  /**
   * The worker could not apply what it was sent and its mirror is no longer
   * trustworthy. The main thread must resend the whole document rather than
   * carry on — ARCHITECTURE §9: silent drift corrupts every offset in the
   * interface.
   */
  | { kind: "desync"; rev: number; reason: string };
