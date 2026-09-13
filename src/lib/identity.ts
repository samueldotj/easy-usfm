/**
 * What this file says it is.
 *
 * The document bar names three things: the book, the translation and the file.
 * Only the last is something the application knows on its own — the other two
 * are written in the header of every USFM file in existence and were simply
 * never read.
 *
 * - `\h` is the running header, which is the book's name as this translation
 *   spells it. `\toc2` and `\toc1` are the short and long forms, and `\mt1` is
 *   the title as printed. Any of them is better than `43_JHNBSB.usfm`.
 * - `\id` carries the book code and, by long convention, a description after
 *   it: `\id JHN - Berean Standard Bible`.
 *
 * Read off the source rather than the parse for one reason: this is wanted on
 * the first frame, before the engine has answered, and a title bar that
 * appears a beat after everything else is a title bar that flickers. It only
 * ever looks at the head of the file.
 */

/** How far in the header can be. Generous; nothing legitimate is further. */
const HEADER_LINES = 60;

export interface Identity {
  /** The book, as this translation names it. */
  book: string | null;
  /** The translation, where `\id` names one. */
  translation: string | null;
  /** The three-letter book code, which is the one unambiguous thing. */
  code: string | null;
}

/** In order of preference: the name a reader would recognise comes first. */
const NAMES = ["h", "toc2", "toc1", "mt1", "mt"];

export function identityOf(text: string): Identity {
  const found = new Map<string, string>();
  let code: string | null = null;
  let translation: string | null = null;

  let start = 0;
  for (let line = 0; line < HEADER_LINES; line += 1) {
    const newline = text.indexOf("\n", start);
    const source = text.slice(start, newline === -1 ? undefined : newline);

    const match = /^\s*\\([a-z]+[0-9]*)\s+(.+?)\s*$/.exec(source);
    if (match) {
      const [, marker = "", value = ""] = match;
      if (marker === "id") {
        // `JHN - Berean Standard Bible`, or just `JHN`. The separator is a
        // convention rather than a rule, so anything after the code counts and
        // a leading dash is trimmed off it.
        const [, first = "", rest = ""] = /^(\S+)\s*(.*)$/.exec(value) ?? [];
        code = first;
        const described = rest.replace(/^[-–—:]\s*/, "").trim();
        if (described) translation = described;
      } else if (NAMES.includes(marker) && !found.has(marker)) {
        found.set(marker, value);
      }
    }

    if (newline === -1) break;
    start = newline + 1;
  }

  const book = NAMES.map((marker) => found.get(marker)).find((value) => value) ?? null;
  return { book, translation, code };
}
