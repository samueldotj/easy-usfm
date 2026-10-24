/**
 * Inserting USFM markers, from the toolbar or the menu.
 *
 * One table, two ways to reach it. The toolbar and the native menu offer the
 * same commands, and a second copy of "what does Bold insert" is a second thing
 * to keep correct — the menu would eventually wrap with `\bd` while the toolbar
 * wrapped with `\b`, and nobody would notice until a file was wrong.
 *
 * # Two kinds of marker, and the difference matters
 *
 * USFM has **character** markers, which wrap a span of text inside a paragraph
 * (`\bd bold\bd*`), and **paragraph** markers, which begin a line and own
 * everything after them (`\q1`, `\c 3`). Inserting a paragraph marker in the
 * middle of a line does not produce a paragraph — it produces a line with a
 * marker halfway through it, which the parser reads as exactly that.
 *
 * So a paragraph insertion opens a new line first when the caret is not already
 * at one, and a character insertion never does. That single rule is why this is
 * a table rather than a set of strings scattered through a component.
 *
 * # Numbers are suggested, not imposed
 *
 * `\c` and `\v` take a number, and the useful one is almost always the next.
 * They are given the next number where it can be worked out, with the caret
 * placed *after* it so typing a different one is one keystroke — rather than
 * inserting a bare marker and making the common case the one that costs work.
 */

/** What a command does to the document. */
export interface Insertion {
  /** Replaces the selected range with this. */
  text: string;
  /**
   * Where the caret goes afterwards, as an offset into `text`.
   *
   * Named rather than derived, because it differs by command: inside the pair
   * for `\bd`, after the number for `\c`, at the end for `\q1`.
   */
  caret: number;
  /** How much of `text` to select, from `caret`. Zero places a bare caret. */
  select?: number;
}

/** One command, as the toolbar and the menu both see it. */
export interface Command {
  /** The menu id, and the toolbar's key. */
  id: string;
  /** What the button says to a screen reader, and shows on hover. */
  label: string;
  /** The longer form, for the tooltip. Says what USFM it writes. */
  help: string;
  /** Whether it must start a line (ARCHITECTURE: paragraph vs character). */
  paragraph: boolean;
}

export const COMMANDS: readonly Command[] = [
  {
    id: "insert-chapter",
    label: "Chapter",
    help: "Chapter — inserts \\c on a new line",
    paragraph: true,
  },
  {
    id: "insert-verse",
    label: "Verse",
    help: "Verse — inserts \\v at the caret",
    paragraph: false,
  },
  {
    id: "insert-bold",
    label: "Bold",
    help: "Bold — wraps the selection in \\bd … \\bd*",
    paragraph: false,
  },
  {
    id: "insert-italic",
    label: "Italic",
    help: "Italic — wraps the selection in \\it … \\it*",
    paragraph: false,
  },
  {
    id: "insert-paragraph",
    label: "Paragraph",
    help: "Paragraph — inserts \\p on a new line",
    paragraph: true,
  },
  {
    id: "insert-section",
    label: "Section heading",
    help: "Section heading — inserts \\s1 on a new line",
    paragraph: true,
  },
  {
    id: "insert-parallel",
    label: "Parallel references",
    help: "Parallel references — inserts \\r, the line under a section heading",
    paragraph: true,
  },
  {
    id: "insert-footnote",
    label: "Footnote",
    help: "Footnote — inserts \\f + \\fr … \\ft … \\f* at the caret",
    paragraph: false,
  },
  {
    id: "insert-xref",
    label: "Cross reference",
    help: "Cross reference — inserts \\x + \\xo … \\xt … \\x* at the caret",
    paragraph: false,
  },
  {
    id: "insert-sidebar",
    label: "Study sidebar",
    help: "Study sidebar — inserts an \\esb … \\esbe block",
    paragraph: true,
  },
  {
    id: "insert-break",
    label: "Blank line",
    help: "Blank line — inserts \\b, the space between stanzas",
    paragraph: true,
  },
  {
    id: "insert-poetry",
    label: "Poetry line",
    help: "Poetry line — inserts \\q1 on a new line",
    paragraph: true,
  },
  {
    id: "insert-table",
    label: "Table",
    help: "Table — inserts \\tr rows with \\th header and \\tc body cells",
    paragraph: true,
  },
  {
    id: "insert-figure",
    label: "Image",
    help: "Image — inserts \\fig with a file to fill in",
    paragraph: true,
  },
] as const;

/** What the document is, as far as an insertion needs to know. */
export interface Where {
  /** The whole text, to see whether the caret is at the start of a line. */
  text: string;
  /** The selected range, or a caret when the two are equal. */
  from: number;
  to: number;
  /** The next chapter number, where one can be worked out. */
  nextChapter?: number;
  /** The next verse number, likewise. */
  nextVerse?: number;
  /**
   * Where the caret is, as the engine reports it — `JHN 1:5` or `1:5`.
   *
   * Only a note wants this, and only for its `\fr`. A footnote whose reference
   * is already filled in is the difference between one keystroke and having to
   * remember which verse you were in.
   */
  reference?: string;
}

/**
 * Whether `at` begins a line.
 *
 * Offset zero counts: the start of the document is the start of its first line,
 * and a paragraph marker there needs no newline in front of it.
 */
function atLineStart(text: string, at: number): boolean {
  return at === 0 || text[at - 1] === "\n";
}

/**
 * Turns a command and a position into an edit.
 *
 * `null` for an id this does not know, so a menu item that has lost its command
 * does nothing rather than inserting something arbitrary.
 */
export function insertionFor(id: string, where: Where): Insertion | null {
  const selected = where.text.slice(where.from, where.to);

  // A paragraph marker needs a line of its own. Opened here rather than in each
  // arm so no command can forget it.
  const command = COMMANDS.find((entry) => entry.id === id);
  if (!command) return null;
  const lead = command.paragraph && !atLineStart(where.text, where.from) ? "\n" : "";

  const shift = (insertion: Insertion): Insertion => ({
    ...insertion,
    text: lead + insertion.text,
    caret: lead.length + insertion.caret,
  });

  switch (id) {
    case "insert-chapter": {
      // The number, then a newline: a chapter marker owns its own line and the
      // text that follows belongs to the next one.
      const marker = numbered("c", where.nextChapter);
      const text = `${marker}\n`;
      // After the newline, on the line below — not after the number.
      //
      // Leaving it on the chapter line so the number could be retyped read
      // well and produced `\c 1\v 1` when the next thing pressed was Verse,
      // which is the commonest pair there is: a verse marker legitimately sits
      // inline, so nothing downstream objected. The number is already the right
      // one almost always; continuing to write is the case worth optimising.
      return shift({ text, caret: text.length });
    }

    case "insert-verse": {
      // A trailing space, because verse text follows on the same line — but
      // only one. Building this as `\v ${number} ` produced `\v  ` when no
      // number could be worked out, and a stray double space is the sort of
      // thing that turns up in somebody's diff months later.
      const text = `${numbered("v", where.nextVerse)} `;
      return shift({ text, caret: text.length });
    }

    case "insert-bold":
      return wrap("bd", selected);

    case "insert-italic":
      return wrap("it", selected);

    case "insert-paragraph":
      return shift({ text: "\\p\n", caret: 3 });

    case "insert-break":
      // `\b` is a blank line between stanzas, and takes no text of its own --
      // so the caret goes to the line after it, where the next thing goes.
      return shift({ text: "\\b\n", caret: 3 });

    case "insert-poetry":
      return shift({ text: "\\q1 ", caret: 4 });

    case "insert-table": {
      // A header row and one body row, because a table with only headers is
      // not a table and leaves the user to guess the row marker.
      const text = "\\tr \\th1 \\th2 \n\\tr \\tc1 \\tc2 \n";
      // In the first header cell, which is where the typing starts.
      return shift({ text, caret: "\\tr \\th1 ".length });
    }

    case "insert-section":
      return shift({ text: "\\s1 ", caret: 4 });

    case "insert-parallel":
      // The italic line of parallel passages that sits under a section
      // heading. Its own line, and almost always directly after one.
      return shift({ text: "\\r ", caret: 3 });

    case "insert-footnote":
      return note("f", "fr", "ft", selected, where.reference);

    case "insert-xref":
      return note("x", "xo", "xt", selected, where.reference);

    case "insert-sidebar": {
      // Written out rather than built from `newSidebar`, because the caret has
      // to land on the heading and an empty `\ms` is not something a finished
      // sidebar should carry. `sidebars.test.ts` checks that what this writes
      // reads back as a sidebar, which is the coupling that matters: the two
      // have to agree on the shape, not on the string.
      const before = "\\esb\n\\ms ";
      const text = `${before}\n\\p \n\\esbe\n`;
      return shift({ text, caret: before.length });
    }

    case "insert-figure": {
      // The caption position first, then the attributes. `src` is left empty
      // and selected: it is the one part that cannot be guessed, and it is
      // what the user has to supply for the figure to show at all.
      const before = "\\fig ";
      const after = '|src="" size="col"\\fig*\n';
      const text = `${before}${after}`;
      return shift({ text, caret: before.length + '|src="'.length });
    }

    default:
      return null;
  }
}

/**
 * A marker with its number, or without one when none could be worked out.
 *
 * The caret lands where the number goes either way, so typing one is the same
 * gesture whether or not a suggestion was available.
 */
function numbered(marker: string, number: number | undefined): string {
  return number === undefined ? `\\${marker} ` : `\\${marker} ${number}`;
}

/**
 * The command that writes a given marker, where there is one.
 *
 * The marker strip and the palette's `\` mode both let a marker be picked by
 * name, and for a dozen of them there is already a command that knows how to
 * write it properly — with its number suggested, its pair closed, its block
 * laid out. Routing through the command is what stops `\c` inserted from the
 * strip behaving differently from `\c` inserted from the toolbar.
 */
const BY_MARKER: Record<string, string> = {
  c: "insert-chapter",
  v: "insert-verse",
  p: "insert-paragraph",
  s1: "insert-section",
  s: "insert-section",
  r: "insert-parallel",
  q1: "insert-poetry",
  b: "insert-break",
  f: "insert-footnote",
  x: "insert-xref",
  esb: "insert-sidebar",
  tr: "insert-table",
  fig: "insert-figure",
  bd: "insert-bold",
  it: "insert-italic",
};

export function commandForMarker(marker: string): string | null {
  return BY_MARKER[marker] ?? null;
}

/** What the marker table says a marker is. */
export type MarkerClass = "character" | "paragraph" | "note" | "milestone" | "unclassified";

const CLASSES: readonly string[] = [
  "character",
  "paragraph",
  "note",
  "milestone",
  "unclassified",
];

/**
 * The marker table's `class`, narrowed.
 *
 * It arrives from the engine as a string, because the wire format is data and
 * not a type. A class this side does not recognise becomes "unclassified",
 * which is the honest answer and the one whose insertion shape is safest.
 */
export function markerClass(value: string | undefined): MarkerClass {
  return CLASSES.includes(value ?? "") ? (value as MarkerClass) : "unclassified";
}

/**
 * An insertion for any marker at all, from what class it is.
 *
 * The fallback behind {@link commandForMarker}: USFM has several hundred
 * markers and this application will never have a hand-written command for each
 * one. What it can do is get the *shape* right, which is the part that decides
 * whether the file still parses — a character marker wraps and closes, a
 * paragraph marker takes a line of its own, a milestone is self-closing.
 */
export function insertionForMarker(
  marker: string,
  kind: MarkerClass,
  where: Where,
): Insertion {
  const known = commandForMarker(marker);
  if (known) {
    const insertion = insertionFor(known, where);
    if (insertion) return insertion;
  }

  const selected = where.text.slice(where.from, where.to);

  if (kind === "character") return wrap(marker, selected);

  if (kind === "note") {
    // A caller, because a note without one is a diagnostic. `+` is USFM for
    // "number it for me", which is what every published edition uses.
    const open = `\\${marker} + `;
    const close = `\\${marker}*`;
    return { text: `${open}${selected}${close}`, caret: open.length, select: selected.length };
  }

  if (kind === "milestone") {
    // Self-closing, and it marks a position rather than containing anything --
    // so the selection is left alone and the marker goes in front of it.
    const text = `\\${marker}\\*`;
    return { text: `${text}${selected}`, caret: text.length, select: selected.length };
  }

  // Paragraph, and anything unclassified: a line of its own is the safe shape,
  // because a paragraph marker halfway through a line is read as exactly that.
  const lead = atLineStart(where.text, where.from) ? "" : "\n";
  const text = `${lead}\\${marker} `;
  return { text: `${text}${selected}`, caret: text.length, select: selected.length };
}

/**
 * A note around the selection — a footnote or a cross-reference.
 *
 * The caller is `+`, which is USFM for "number it for me". A translator
 * choosing their own caller character is the rare case; the automatic one is
 * what every published edition uses.
 *
 * The reference is filled in from where the caret is when the engine knows,
 * and left out entirely when it does not — rather than inserting `\fr ` with
 * nothing after it, which is a marker with no value and a diagnostic waiting
 * to happen.
 */
function note(
  outer: string,
  referenceMarker: string,
  textMarker: string,
  selected: string,
  reference: string | undefined,
): Insertion {
  const at = chapterVerse(reference);
  const open =
    `\\${outer} + ` + (at === null ? "" : `\\${referenceMarker} ${at} `) + `\\${textMarker} `;
  const close = `\\${outer}*`;

  return {
    text: `${open}${selected}${close}`,
    caret: open.length,
    select: selected.length,
  };
}

/**
 * The chapter and verse out of a reference, without the book code.
 *
 * `\fr` names a place within this book, so `JHN 1:5` has to become `1:5` — the
 * book code belongs in a cross-reference's target, not in a footnote's own
 * back-reference. A reference with no colon is a chapter the caret is in with
 * no verse yet, which is not somewhere a note's reference can point.
 */
export function chapterVerse(reference: string | undefined): string | null {
  if (!reference) return null;
  const last = reference.trim().split(/\s+/).at(-1) ?? "";
  return last.includes(":") ? last : null;
}

/**
 * A character marker around the selection.
 *
 * With nothing selected the pair is still inserted and the caret goes between
 * them, which is what someone who pressed Bold before typing expects. With a
 * selection the text is kept and wrapped — never replaced, which is the one
 * outcome that loses work.
 */
function wrap(marker: string, selected: string): Insertion {
  const open = `\\${marker} `;
  const close = `\\${marker}*`;

  return {
    text: `${open}${selected}${close}`,
    caret: open.length,
    // The wrapped text stays selected, so the next command applies to it too
    // and Bold-then-Italic does what it looks like it does.
    select: selected.length,
  };
}
