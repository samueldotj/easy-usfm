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
