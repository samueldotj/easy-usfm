/**
 * Study sidebars — `\esb … \esbe` — read as structure and written back as text.
 *
 * A sidebar is the one place in USFM where a translator is really editing a
 * small record rather than a stream: it has a category, a heading and a body,
 * and every study Bible has hundreds of them. The redesign gives them fields,
 * and this is the part that has to be right, because it is the only code in
 * the application that rewrites a span of somebody's file from a form.
 *
 * # It reads the source, not the tree
 *
 * Everywhere else the engine is the authority on what a document says, and
 * this does not change that: the panel finds its blocks inside the chapter the
 * *parse* says is current, and every offset it hands back goes through the
 * editor's own transaction. What it cannot get from the USJ tree is where the
 * text of one field ends and the next begins — a `\cat` value's span is not
 * something the tree records — and an editor that guesses at that is an editor
 * that eventually overwrites the wrong twelve characters. `\esb`, `\esbe`,
 * `\cat` and `\ms` are line-level by specification, so reading the lines is
 * exact rather than approximate.
 *
 * Scoped to a chapter, never to the document. The panel shows "3 in chapter 1"
 * because that is the question being asked, and scanning two megabytes on
 * every keystroke to answer it would be absurd.
 *
 * # Nothing is reordered and nothing is dropped
 *
 * A real sidebar contains more than three markers — poetry, lists, a figure,
 * a second heading. The obvious implementation reads the fields it knows,
 * serialises them in a canonical order and writes that back, which quietly
 * deletes everything else. So the rewrite is *positional*: each original line
 * is replaced in place by its edited form, lines this does not model pass
 * through untouched and in order, and only genuinely new content is inserted
 * at a chosen point. ADR-003 — content survives — is the rule, and a form is
 * not an excuse to break it.
 */

/** One line of source, with where it is. */
interface Line {
  text: string;
  /** Offset of the first character, relative to the string scanned. */
  start: number;
  /** Offset just past the last character, before its newline. */
  end: number;
  /** 1-based, counting from the start of the string scanned. */
  number: number;
}

/** What kind of line it is, as far as a sidebar cares. */
type Role = "open" | "close" | "cat" | "ms" | "p" | "other";

interface Inner extends Line {
  role: Role;
  /** The value after the marker, for the roles that have one. */
  value: string;
}

/** One `\esb … \esbe` block. */
export interface Sidebar {
  /** Position among the sidebars of this chapter, from zero. */
  index: number;
  /** Offset of the backslash of `\esb`, in the document. */
  start: number;
  /** Offset just past the `e` of `\esbe`, in the document. */
  end: number;
  /** 1-based line numbers, for the "L16–20" the panel shows. */
  firstLine: number;
  lastLine: number;
  /** `\cat`, or `null` where the block has none. */
  category: string | null;
  /** The first `\ms`, which is the sidebar's title. */
  heading: string | null;
  /** Every `\p`, in order. */
  paragraphs: string[];
  /** The reference this sits after, as `chapter:verse`, or `null`. */
  after: string | null;
  /** How many lines it contains that this does not model. */
  otherLines: number;
  /** The lines themselves, kept for the rewrite. Not for display. */
  readonly lines: readonly Inner[];
}

/** The editable fields, as the panel hands them back. */
export interface SidebarEdit {
  category: string | null;
  heading: string | null;
  paragraphs: string[];
}

/** Splits a string into lines that remember where they were. */
function linesOf(text: string): Line[] {
  const lines: Line[] = [];
  let start = 0;
  let number = 1;

  for (;;) {
    const newline = text.indexOf("\n", start);
    const end = newline === -1 ? text.length : newline;
    lines.push({ text: text.slice(start, end), start, end, number });
    if (newline === -1) break;
    start = newline + 1;
    number += 1;
  }

  return lines;
}

/**
 * What a line is.
 *
 * Leading whitespace is tolerated because files in the wild have it, and a
 * sidebar that failed to be recognised because somebody indented it would be a
 * sidebar the panel silently cannot see.
 */
function classify(text: string): { role: Role; value: string } {
  const match = /^\s*\\([a-z]+[0-9]*)\s?([\s\S]*)$/.exec(text);
  if (!match) return { role: "other", value: "" };

  const [, marker = "", rest = ""] = match;
  switch (marker) {
    case "esb":
      return { role: "open", value: "" };
    case "esbe":
      return { role: "close", value: "" };
    case "cat":
      // `\cat Word Study\cat*` — the closing marker is part of the line, not
      // part of the value.
      return { role: "cat", value: rest.replace(/\\cat\*\s*$/, "").trim() };
    case "ms":
    case "ms1":
      return { role: "ms", value: rest.trim() };
    case "p":
      return { role: "p", value: rest };
    default:
      return { role: "other", value: rest };
  }
}

/**
 * Every sidebar in a slice of the document.
 *
 * `base` is where the slice begins, so the offsets that come back address the
 * whole document — the caller slices a chapter and gets positions it can hand
 * straight to the editor.
 *
 * An unclosed `\esb` is not returned. It is a real thing to find in a file
 * being written, and offering fields for a block whose end is unknown means
 * offering to overwrite everything after it.
 */
export function sidebarsIn(slice: string, base = 0, firstLineNumber = 1): Sidebar[] {
  const lines = linesOf(slice);
  const found: Sidebar[] = [];

  let chapter: string | null = null;
  let verse: string | null = null;
  let open: { at: number; after: string | null } | null = null;

  lines.forEach((line, at) => {
    const { role } = classify(line.text);

    if (open === null) {
      // Only tracked outside a block: a `\v` inside a sidebar is that
      // sidebar's own content and does not move the Scripture's position.
      const chapterMark = /^\s*\\c\s+(\S+)/.exec(line.text);
      if (chapterMark?.[1]) {
        chapter = chapterMark[1];
        verse = null;
      }
      const verseMark = /^\s*\\v\s+(\S+)/.exec(line.text);
      if (verseMark?.[1]) verse = verseMark[1];

      if (role === "open") {
        const reference = chapter === null ? verse : verse === null ? chapter : `${chapter}:${verse}`;
        open = { at, after: reference };
      }
      return;
    }

    if (role === "close") {
      const block = lines.slice(open.at, at + 1).map((inner) => ({
        ...inner,
        ...classify(inner.text),
      }));

      found.push(assemble(found.length, block, base, firstLineNumber, open.after));
      open = null;
    }
  });

  return found;
}

function assemble(
  index: number,
  lines: Inner[],
  base: number,
  firstLineNumber: number,
  after: string | null,
): Sidebar {
  const first = lines[0];
  const last = lines[lines.length - 1];
  const category = lines.find((line) => line.role === "cat")?.value ?? null;
  const heading = lines.find((line) => line.role === "ms")?.value ?? null;

  return {
    index,
    start: base + (first?.start ?? 0),
    end: base + (last?.end ?? 0),
    firstLine: firstLineNumber + (first?.number ?? 1) - 1,
    lastLine: firstLineNumber + (last?.number ?? 1) - 1,
    category,
    heading,
    paragraphs: lines.filter((line) => line.role === "p").map((line) => line.value),
    after,
    // The opening and closing markers are not "other" in the sense that
    // matters; they are the block.
    otherLines: lines.filter((line) => line.role === "other").length,
    lines,
  };
}

/**
 * The replacement text for a sidebar's span, with the edited fields in it.
 *
 * Positional, as the module comment says. Walking the original lines rather
 * than rebuilding from the model is what keeps a `\q1` in the middle of a
 * sidebar exactly where its author put it.
 *
 * The three ways a field can move:
 *
 * - **Changed** — the line is rewritten where it stands.
 * - **Cleared** — the line goes. Emptying the category means the block has no
 *   category, and leaving `\cat \cat*` behind would be a marker with no value.
 * - **Added** — a line that did not exist is inserted at the one place the
 *   specification allows it to go: `\cat` and `\ms` belong at the head of the
 *   block, and a new paragraph after the last one.
 */
export function rewrite(sidebar: Sidebar, edit: SidebarEdit): string {
  const out: string[] = [];

  const hadCategory = sidebar.lines.some((line) => line.role === "cat");
  const firstHeading = sidebar.lines.find((line) => line.role === "ms");
  const lastParagraph = findLast(sidebar.lines, (line) => line.role === "p");

  let paragraph = 0;

  for (const line of sidebar.lines) {
    switch (line.role) {
      case "open":
        out.push(line.text);
        // A category the block did not have goes directly under `\esb`, and a
        // heading under that -- which is the order every published study Bible
        // writes them in.
        if (!hadCategory && edit.category) out.push(`\\cat ${edit.category}\\cat*`);
        if (firstHeading === undefined && edit.heading) out.push(`\\ms ${edit.heading}`);
        break;

      case "cat":
        if (edit.category) out.push(`\\cat ${edit.category}\\cat*`);
        break;

      case "ms":
        // Only the first `\ms` is the sidebar's heading; a second one is
        // content and is left alone.
        if (line === firstHeading) {
          if (edit.heading) out.push(`\\ms ${edit.heading}`);
        } else {
          out.push(line.text);
        }
        break;

      case "p": {
        const replacement = edit.paragraphs[paragraph];
        paragraph += 1;
        // Kept even when empty: `\p` with no text is a legal paragraph break
        // and deleting it because a textarea was cleared would change the
        // block's shape rather than its words. Paragraphs are removed by the
        // panel handing back a shorter list, which is the case below.
        if (replacement !== undefined) out.push(`\\p ${replacement}`.trimEnd());
        // Anything past the end of the edited list has been removed.
        if (line === lastParagraph) {
          for (const extra of edit.paragraphs.slice(paragraph)) {
            out.push(`\\p ${extra}`.trimEnd());
          }
        }
        break;
      }

      case "close":
        // A block that had no paragraphs at all still has to be able to gain
        // one, and the only place left is here.
        if (lastParagraph === undefined) {
          for (const extra of edit.paragraphs) out.push(`\\p ${extra}`.trimEnd());
        }
        out.push(line.text);
        break;

      default:
        out.push(line.text);
    }
  }

  return out.join("\n");
}

/**
 * A whole new sidebar, ready to be inserted on its own lines.
 *
 * No trailing newline: the caller knows whether it is opening a line first,
 * which is the same rule every other insertion follows (`lib/insert.ts`).
 */
export function newSidebar(edit: SidebarEdit): string {
  const lines = ["\\esb"];
  if (edit.category) lines.push(`\\cat ${edit.category}\\cat*`);
  if (edit.heading) lines.push(`\\ms ${edit.heading}`);
  for (const paragraph of edit.paragraphs) lines.push(`\\p ${paragraph}`.trimEnd());
  if (edit.paragraphs.length === 0) lines.push("\\p");
  lines.push("\\esbe");
  return lines.join("\n");
}

/** Whether an edit would change anything, so Apply can say so. */
export function changed(sidebar: Sidebar, edit: SidebarEdit): boolean {
  return rewrite(sidebar, edit) !== sidebar.lines.map((line) => line.text).join("\n");
}

/** `Array.prototype.findLast` without requiring the newer library target. */
function findLast<T>(items: readonly T[], matches: (item: T) => boolean): T | undefined {
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const item = items[index];
    if (item !== undefined && matches(item)) return item;
  }
  return undefined;
}

/**
 * How many words a body has, for the panel's count.
 *
 * Whitespace-separated runs. Deliberately naive: it is a rough measure shown
 * beside a text box, and a script-aware word count is a different problem that
 * this is not the place to get half right.
 */
export function wordCount(paragraphs: readonly string[]): number {
  return paragraphs.join(" ").split(/\s+/).filter(Boolean).length;
}
