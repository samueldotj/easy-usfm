/**
 * The navigator's two lists.
 *
 * Both are derived from what the engine already reports, so what is tested
 * here is the derivation and not the parse: which chapter a diagnostic lands
 * in, and which verses a heading turns out to own.
 */

import { describe, expect, it } from "vitest";

import { chapterAt, chaptersOf, outlineOf, sectionAt, textOf, verseRange } from "./outline";
import type { Chunk, Diagnostic, PreviewNode } from "../worker/protocol";

function chunk(number: number | null, start: number, end: number): Chunk {
  return { number, start, end, rev: 1 };
}

function diagnostic(
  start: number,
  severity: Diagnostic["severity"] = "error",
): Diagnostic {
  return { code: "USFM-1", severity, start, end: start + 1, line: 1, message: "" };
}

function node(partial: Partial<PreviewNode>): PreviewNode {
  return {
    kind: "text",
    marker: null,
    attributes: [],
    start: null,
    end: null,
    children: [],
    text: null,
    raw: null,
    ...partial,
  };
}

const text = (value: string) => node({ kind: "text", text: value });
const heading = (marker: string, title: string, start = 0) =>
  node({ kind: "para", marker, start, children: [text(title)] });
const verse = (number: string) =>
  node({ kind: "verse", marker: "v", attributes: [{ key: "number", value: number }] });
const para = (...children: PreviewNode[]) => node({ kind: "para", marker: "p", children });

describe("the chapter grid", () => {
  const chunks = [chunk(null, 0, 100), chunk(1, 100, 300), chunk(2, 300, 500)];

  it("names the material before the first chapter", () => {
    expect(chaptersOf(chunks, []).map((entry) => entry.label)).toEqual(["Front", "1", "2"]);
  });

  it("marks the chapter a diagnostic is in", () => {
    const entries = chaptersOf(chunks, [diagnostic(350)]);
    expect(entries.map((entry) => entry.severity)).toEqual([null, null, "error"]);
  });

  it("shows the worst of several", () => {
    // A chapter with an error and a warning has an error; a dot that showed
    // the last one found would depend on the order they were reported in.
    const entries = chaptersOf(chunks, [
      diagnostic(150, "warning"),
      diagnostic(160, "error"),
      diagnostic(170, "information"),
    ]);
    expect(entries[1]?.severity).toBe("error");
  });

  it("puts a diagnostic at the very end in the last chapter", () => {
    // A chunk's `end` is exclusive, so a diagnostic at the final offset is in
    // no chunk at all -- and must not be silently dropped from the grid.
    expect(chaptersOf(chunks, [diagnostic(500)])[2]?.severity).toBe("error");
  });

  it("survives a document with nothing in it", () => {
    expect(chaptersOf([], [diagnostic(0)])).toEqual([]);
  });

  it("finds the chapter an offset is in", () => {
    const entries = chaptersOf(chunks, []);
    expect(chapterAt(entries, 0)).toBe(0);
    expect(chapterAt(entries, 299)).toBe(1);
    expect(chapterAt(entries, 300)).toBe(2);
    // Past the end is the last chapter, which is where the caret is.
    expect(chapterAt(entries, 9000)).toBe(2);
    expect(chapterAt([], 0)).toBe(-1);
  });
});

describe("the outline", () => {
  const nodes = [
    heading("s1", "The Beginning", 10),
    para(verse("1"), text("In the beginning"), verse("5"), text("The Light")),
    heading("s1", "The Witness of John", 200),
    para(verse("6"), text("There came a man"), verse("8"), text("He himself")),
  ];

  it("lists the headings with the verses under them", () => {
    expect(outlineOf(nodes).map((section) => [section.title, verseRange(section)])).toEqual([
      ["The Beginning", "1–5"],
      ["The Witness of John", "6–8"],
    ]);
  });

  it("shows one number when a section has one verse", () => {
    const one = [heading("s1", "Short", 0), para(verse("7"), text("only"))];
    expect(verseRange(outlineOf(one)[0]!)).toBe("7");
  });

  it("keeps a heading that has no verses yet", () => {
    // Somebody has just typed it. Dropping it means the outline flickers while
    // a section is being written, which is exactly when it is being looked at.
    const empty = outlineOf([heading("s1", "Nothing under this", 0)]);
    expect(empty).toHaveLength(1);
    expect(verseRange(empty[0]!)).toBe("");
  });

  it("records the level so the list can show depth", () => {
    const levels = outlineOf([
      heading("ms", "Major", 0),
      heading("s1", "Section", 1),
      heading("s3", "Deeper", 2),
    ]);
    expect(levels.map((section) => [section.marker, section.level])).toEqual([
      ["ms", 1],
      ["s1", 1],
      ["s3", 3],
    ]);
  });

  it("takes headings and nothing else", () => {
    // `\r` is a parallel-reference line and `\mt1` is the book title. Neither
    // is a section, and both sit right beside ones that are.
    const mixed = outlineOf([
      heading("mt1", "John", 0),
      heading("r", "(Genesis 1:1)", 1),
      heading("s1", "Real", 2),
      heading("sp", "Speaker", 3),
    ]);
    expect(mixed.map((section) => section.title)).toEqual(["Real"]);
  });

  it("uses the published verse number the file wrote", () => {
    // Whatever `number` says. A file using letters for verses shows letters.
    const lettered = outlineOf([heading("s1", "Ch", 0), para(verse("1a"), text("x"))]);
    expect(verseRange(lettered[0]!)).toBe("1a");
  });

  it("finds the section an offset is in", () => {
    const sections = outlineOf(nodes);
    expect(sectionAt(sections, 0)).toBe(-1);
    expect(sectionAt(sections, 10)).toBe(0);
    expect(sectionAt(sections, 199)).toBe(0);
    expect(sectionAt(sections, 500)).toBe(1);
  });

  it("is empty for a chapter that has not rendered yet", () => {
    expect(outlineOf([])).toEqual([]);
  });
});

describe("flattening text", () => {
  it("joins the leaves", () => {
    expect(textOf(para(text("one "), node({ kind: "char", children: [text("two")] })))).toBe(
      "one two",
    );
  });

  it("leaves a footnote out of it", () => {
    // A heading can carry one, and its text is apparatus -- putting "Or
    // comprehended" in the table of contents is the failure this avoids.
    const withNote = para(
      text("Title"),
      node({ kind: "note", marker: "f", children: [text(" Or something")] }),
    );
    expect(textOf(withNote)).toBe("Title");
  });
});
