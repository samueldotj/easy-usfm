/**
 * What the insert commands write.
 *
 * These produce USFM that goes into somebody's translation, so the assertions
 * are on the exact text — a marker with a stray space or a paragraph marker
 * that landed mid-line is not a cosmetic problem, it is a file that parses as
 * something the author did not write.
 */

import { describe, expect, it } from "vitest";

import { COMMANDS, insertionFor, type Where } from "./insert";

/** A caret at the end of `text`, unless a range is given. */
function at(text: string, from = text.length, to = from): Where {
  return { text, from, to };
}

/** The text with a `|` where the caret ends up, which is what a user sees. */
function withCaret(id: string, where: Where): string {
  const insertion = insertionFor(id, where);
  if (!insertion) return "(none)";

  const { text, caret, select = 0 } = insertion;
  return select === 0
    ? `${text.slice(0, caret)}|${text.slice(caret)}`
    : `${text.slice(0, caret)}[${text.slice(caret, caret + select)}]${text.slice(caret + select)}`;
}

describe("character markers", () => {
  it("wraps the selection and keeps it selected", () => {
    // Keeping it selected is what makes Bold-then-Italic do what it looks
    // like it does.
    expect(withCaret("insert-bold", at("the word here", 4, 8))).toBe("\\bd [word]\\bd*");
    expect(withCaret("insert-italic", at("the word here", 4, 8))).toBe("\\it [word]\\it*");
  });

  it("never loses the text it wraps", () => {
    const insertion = insertionFor("insert-bold", at("keep me", 0, 7));
    expect(insertion?.text).toContain("keep me");
  });

  it("puts the caret between the pair when nothing is selected", () => {
    expect(withCaret("insert-bold", at(""))).toBe("\\bd |\\bd*");
  });

  it("does not open a line, because it is inline", () => {
    // A character marker mid-sentence is exactly where it belongs.
    expect(insertionFor("insert-bold", at("mid sentence"))?.text.startsWith("\n")).toBe(false);
  });
});

describe("paragraph markers", () => {
  it("open a line when the caret is not at one", () => {
    // Without this the marker lands halfway through a line, and the parser
    // reads it as exactly that rather than as a new paragraph.
    expect(insertionFor("insert-poetry", at("some text"))?.text).toBe("\n\\q1 ");
  });

  it("do not open one when the caret already begins a line", () => {
    expect(insertionFor("insert-poetry", at("some text\n"))?.text).toBe("\\q1 ");
  });

  it("treat the start of the document as the start of a line", () => {
    expect(insertionFor("insert-paragraph", at(""))?.text).toBe("\\p\n");
  });
});

describe("numbers", () => {
  it("suggest the next chapter, leaving the caret on the line below", () => {
    // Not after the number. Chapter then Verse is the commonest pair there is,
    // and a caret left on the chapter line produced `\c 1\v 1` — which nothing
    // downstream objects to, because a verse marker legitimately sits inline.
    const where = { ...at(""), nextChapter: 4 };
    expect(withCaret("insert-chapter", where)).toBe("\\c 4\n|");
  });

  it("puts a chapter and then a verse on separate lines", () => {
    const chapter = insertionFor("insert-chapter", { ...at(""), nextChapter: 2 });
    const after = chapter?.text ?? "";
    const verse = insertionFor("insert-verse", { ...at(after), nextVerse: 1 });
    expect(after + (verse?.text ?? "")).toBe("\\c 2\n\\v 1 ");
  });

  it("suggest the next verse", () => {
    const where = { ...at(""), nextVerse: 12 };
    expect(withCaret("insert-verse", where)).toBe("\\v 12 |");
  });

  it("leave a single space when no number can be worked out", () => {
    // Built naively this produced `\v  ` -- a stray double space, which is the
    // sort of thing that turns up in somebody's diff months later.
    expect(insertionFor("insert-verse", at(""))?.text).toBe("\\v  ");
    expect(insertionFor("insert-chapter", at(""))?.text).toBe("\\c \n");
  });
});

describe("the larger structures", () => {
  it("give a table a header row and a body row", () => {
    // A table with only headers is not a table, and leaves the user to guess
    // the row marker.
    const text = insertionFor("insert-table", at(""))?.text ?? "";
    expect(text).toContain("\\tr \\th1 \\th2");
    expect(text).toContain("\\tr \\tc1 \\tc2");
  });

  it("put the caret in a figure's src, which is the part that cannot be guessed", () => {
    expect(withCaret("insert-figure", at(""))).toBe('\\fig |src="|" size="col"\\fig*\n');
  });
});

describe("the command table", () => {
  it("gives every command an insertion", () => {
    // A button with no insertion is a button that does nothing.
    for (const command of COMMANDS) {
      expect(insertionFor(command.id, at("")), command.id).not.toBeNull();
    }
  });

  it("gives every command a label and help that names its marker", () => {
    for (const command of COMMANDS) {
      expect(command.label.length, command.id).toBeGreaterThan(0);
      expect(command.help, command.id).toContain("\\");
    }
  });

  it("refuses an id it does not know", () => {
    // A menu item whose command was removed should do nothing rather than
    // insert something arbitrary.
    expect(insertionFor("insert-nonsense", at(""))).toBeNull();
  });
});
