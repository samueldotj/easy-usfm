/**
 * What is in the book, as the navigator shows it.
 *
 * Two lists, both derived rather than stored: the chapters, and the sections
 * inside the one being read. Derived is the whole point — the navigator is a
 * view of the parse, so a chapter that has just been typed appears in it
 * without anything being told to refresh, and a heading that has just been
 * deleted leaves.
 *
 * # Why the chapter grid needs the diagnostics
 *
 * The mock-up puts a dot on a chapter that has something wrong with it, and
 * that dot is the reason the grid earns its column: twenty-one numbers with no
 * dots is a table of contents, and twenty-one numbers with one dot is a place
 * to go next. Working out which chapter an offset falls in is this side's job
 * because the engine reports diagnostics against the document, not against a
 * chunk.
 *
 * # Why the outline can be empty
 *
 * Sections come from a chapter's rendered nodes, and chapters render on demand
 * (ARCHITECTURE §10). A chapter nobody has scrolled to has no nodes yet, so it
 * has no outline yet — which is correct, and is why every function here takes
 * what it is given rather than reaching for the engine.
 */

import type { Chunk, Diagnostic, PreviewNode } from "../worker/protocol";

/** Worst-first, because a chapter with an error and a warning has an error. */
export type Severity = Diagnostic["severity"];

const RANK: Record<Severity, number> = { error: 3, warning: 2, information: 1 };

/** One cell in the chapter grid. */
export interface ChapterEntry {
  /** The chunk index, which is what the preview and the engine both key on. */
  index: number;
  /** The chapter number, or `null` for the material before the first `\c`. */
  number: number | null;
  /** What the cell says. */
  label: string;
  start: number;
  end: number;
  /** The worst diagnostic inside it, or `null` when it is clean. */
  severity: Severity | null;
}

/**
 * The chapter grid.
 *
 * Diagnostics are placed by their start offset. A diagnostic that spans a
 * chapter boundary is attributed to the chapter it begins in, which is where
 * the reader will be taken when they follow it.
 */
export function chaptersOf(
  chunks: readonly Chunk[],
  diagnostics: readonly Diagnostic[],
): ChapterEntry[] {
  const entries: ChapterEntry[] = chunks.map((chunk, index) => ({
    index,
    number: chunk.number,
    label: chunk.number === null ? "Front" : String(chunk.number),
    start: chunk.start,
    end: chunk.end,
    severity: null,
  }));

  for (const diagnostic of diagnostics) {
    const at = entries.findIndex(
      (entry) => diagnostic.start >= entry.start && diagnostic.start < entry.end,
    );
    // A diagnostic past the last chunk's end -- at the very end of the
    // document, where a chunk's `end` is exclusive -- belongs to the last one
    // rather than to nothing.
    const entry = entries[at] ?? entries[entries.length - 1];
    if (!entry) continue;

    if (entry.severity === null || RANK[diagnostic.severity] > RANK[entry.severity]) {
      entry.severity = diagnostic.severity;
    }
  }

  return entries;
}

/** One heading in the chapter being read. */
export interface Section {
  /** The heading's own text, flattened. */
  title: string;
  /** The marker that made it, so the list can show its level. */
  marker: string;
  /** 1 for `\s1` and `\ms`, 2 for `\s2`, and so on. Major sections sort above. */
  level: number;
  /** Where the heading is, so clicking it can move the caret. */
  start: number;
  /** The verses it covers, as the numbers the file wrote them. */
  from: string | null;
  to: string | null;
}

/** Headings, and only headings: `\s`, `\s1`..`\s4`, `\ms`, `\ms1`..`\ms3`. */
const HEADING = /^(m?s)([1-9])?$/;

/**
 * The sections of one chapter, in document order.
 *
 * The verse range is accumulated as the walk goes: a section owns every verse
 * between it and the next one. That is how a printed Bible's contents page is
 * built, and it is the only definition available — USFM does not record where
 * a section ends, only where the next one starts.
 */
export function outlineOf(nodes: readonly PreviewNode[]): Section[] {
  const sections: Section[] = [];
  let current: Section | null = null;

  const walk = (list: readonly PreviewNode[]): void => {
    for (const node of list) {
      if (node.kind === "verse") {
        const number = node.attributes.find((entry) => entry.key === "number")?.value;
        if (number !== undefined && current) {
          current.from ??= number;
          current.to = number;
        }
        continue;
      }

      const match = node.kind === "para" && node.marker ? HEADING.exec(node.marker) : null;
      if (match) {
        current = {
          title: textOf(node).trim(),
          marker: node.marker ?? "s",
          // `\ms` is a major section and sits above `\s`; an unnumbered marker
          // is its family's first level.
          level: Number(match[2] ?? "1"),
          start: node.start ?? 0,
          from: null,
          to: null,
        };
        sections.push(current);
        continue;
      }

      walk(node.children);
    }
  };

  walk(nodes);
  // A heading with no verses under it is still a heading -- an empty section
  // someone is about to fill in -- so nothing is dropped here.
  return sections;
}

/** A section's verses, as the label the navigator shows. */
export function verseRange(section: Section): string {
  if (section.from === null) return "";
  if (section.to === null || section.to === section.from) return section.from;
  return `${section.from}–${section.to}`;
}

/**
 * Which section an offset is in.
 *
 * The last one that begins at or before it. `-1` when the offset is above the
 * first heading, which is a real position -- the material between `\c` and the
 * first `\s` belongs to no section.
 */
export function sectionAt(sections: readonly Section[], offset: number): number {
  let found = -1;
  for (let index = 0; index < sections.length; index += 1) {
    const section = sections[index];
    if (section && section.start <= offset) found = index;
    else break;
  }
  return found;
}

/**
 * Which chapter an offset is in, as an index into {@link chaptersOf}.
 *
 * `-1` only when there are no chunks at all; an offset past the end belongs to
 * the last chapter, because that is where the caret is.
 */
export function chapterAt(entries: readonly ChapterEntry[], offset: number): number {
  if (entries.length === 0) return -1;
  const found = entries.findIndex((entry) => offset >= entry.start && offset < entry.end);
  return found === -1 ? entries.length - 1 : found;
}

/**
 * Every text leaf under a node, joined.
 *
 * Used for a heading's title and a sidebar's fields. Notes are skipped: a
 * footnote inside a heading is apparatus, and pulling its text into the
 * outline entry would put "Or comprehended" in the table of contents.
 */
export function textOf(node: PreviewNode): string {
  const parts: string[] = [];

  const walk = (list: readonly PreviewNode[]): void => {
    for (const child of list) {
      if (child.kind === "note") continue;
      if (child.text !== null) parts.push(child.text);
      walk(child.children);
    }
  };

  if (node.text !== null) parts.push(node.text);
  walk(node.children);
  return parts.join("");
}
