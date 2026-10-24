/**
 * Footnotes and cross-references, as the inspector shows them.
 *
 * A note is the other small record USFM contains: a caller, a reference and
 * some text, written inline in the middle of a sentence where it is hardest to
 * read and hardest to edit. The inspector floats it beside the line it is on
 * and shows the three parts separately.
 *
 * Read from the parse rather than from the source, unlike the sidebars: a note
 * is a character-level construct, the tree already separates `\fr` from `\ft`,
 * and nothing here rewrites a *field* — the one edit offered is converting the
 * whole note between a footnote and a cross-reference, which is a rewrite of
 * the span the tree does record.
 */

import { textOf } from "./outline";
import type { PreviewNode } from "../worker/protocol";

/** One note, flattened into the three things the inspector shows. */
export interface NoteEntry {
  /** Position among the notes of the chapter, from zero. */
  index: number;
  /** `f`, `fe`, `ef`, `x`, `ex` — whichever the file wrote. */
  marker: string;
  /** `+` for an automatic caller, `-` for none, or the character itself. */
  caller: string;
  /** `\fr` or `\xo`, which is the verse the note hangs off. */
  reference: string | null;
  /** `\ft` and `\xt` and everything else, joined — the note as it reads. */
  text: string;
  /** The whole note's span, where the parser recorded one. */
  start: number | null;
  end: number | null;
  /** Cross-references are apparatus rather than annotation, and print apart. */
  isReference: boolean;
}

/** The reference marker inside each kind of note. */
const REFERENCE_MARKERS = new Set(["fr", "xo"]);

/** Everything that is the note's own body rather than its machinery. */
const SKIP = new Set(["fr", "xo", "fq", "fqa", "fk", "fl", "fw", "fp", "fv", "fdc"]);

/**
 * Every note in a chapter, in document order.
 *
 * Notes inside notes are not descended into: a note within a note is part of
 * that note's text, not a second entry in the list — the same rule the print
 * collection follows.
 */
export function notesIn(nodes: readonly PreviewNode[]): NoteEntry[] {
  const found: NoteEntry[] = [];

  const walk = (list: readonly PreviewNode[]): void => {
    for (const node of list) {
      if (node.kind === "note") {
        found.push(entryFor(node, found.length));
        continue;
      }
      walk(node.children);
    }
  };

  walk(nodes);
  return found;
}

function entryFor(node: PreviewNode, index: number): NoteEntry {
  const marker = node.marker ?? "f";
  const reference = node.children.find(
    (child) => child.marker !== null && REFERENCE_MARKERS.has(child.marker),
  );

  const body = node.children
    .filter((child) => child.marker === null || !SKIP.has(child.marker))
    .map((child) => textOf(child))
    .join("")
    .trim();

  return {
    index,
    marker,
    caller: node.attributes.find((entry) => entry.key === "caller")?.value ?? "+",
    reference: reference ? textOf(reference).trim() || null : null,
    text: body,
    start: node.start,
    end: node.end,
    isReference: marker === "x" || marker === "ex",
  };
}

/**
 * The markers that change when a footnote becomes a cross-reference.
 *
 * Only the ones with a counterpart. A `\fq` inside a note being converted has
 * no cross-reference equivalent, so it is left as it is rather than mapped to
 * something approximate — a marker that survives conversion unchanged is
 * visible and fixable; one silently turned into a different marker is neither.
 */
const TO_REFERENCE: Record<string, string> = { f: "x", fe: "x", fr: "xo", ft: "xt" };
const TO_FOOTNOTE: Record<string, string> = { x: "f", ex: "f", xo: "fr", xt: "ft" };

/**
 * A note's source, with its markers converted to the other kind.
 *
 * `null` where there is nothing to do, so the caller can leave the document
 * alone rather than dispatching a transaction that changes nothing.
 *
 * Textual, over the note's own span. The alternative — rebuilding the note
 * from the parsed fields — would drop whatever the tree did not model, which
 * for notes is most of what a translator puts in them.
 */
export function convert(source: string): string | null {
  const isFootnote = /^\\(f|fe)\b/.test(source);
  const isReference = /^\\(x|ex)\b/.test(source);
  if (!isFootnote && !isReference) return null;

  const map = isFootnote ? TO_REFERENCE : TO_FOOTNOTE;

  const out = source.replace(/\\([a-z]+[0-9]*)(\*?)/g, (whole, marker: string, star: string) => {
    const replacement = map[marker];
    return replacement === undefined ? whole : `\\${replacement}${star}`;
  });

  return out === source ? null : out;
}

/** Which note an offset is inside, or `-1`. */
export function noteAt(notes: readonly NoteEntry[], offset: number): number {
  return notes.findIndex(
    (note) => note.start !== null && note.end !== null && offset >= note.start && offset <= note.end,
  );
}
