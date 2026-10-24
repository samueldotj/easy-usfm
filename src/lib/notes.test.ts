/**
 * Reading notes out of the parse, and the one edit the inspector offers.
 */

import { describe, expect, it } from "vitest";

import { convert, noteAt, notesIn } from "./notes";
import type { PreviewNode } from "../worker/protocol";

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
const field = (marker: string, value: string) =>
  node({ kind: "char", marker, children: [text(value)] });

const footnote = (start = 100) =>
  node({
    kind: "note",
    marker: "f",
    start,
    end: start + 40,
    attributes: [{ key: "caller", value: "+" }],
    children: [field("fr", "1:5 "), field("ft", "Or comprehended")],
  });

describe("reading notes", () => {
  it("separates the caller, the reference and the text", () => {
    const [note] = notesIn([node({ kind: "para", marker: "p", children: [footnote()] })]);

    expect(note?.caller).toBe("+");
    expect(note?.reference).toBe("1:5");
    expect(note?.text).toBe("Or comprehended");
    expect(note?.isReference).toBe(false);
  });

  it("knows a cross-reference from a footnote", () => {
    const cross = node({
      kind: "note",
      marker: "x",
      children: [field("xo", "3:16 "), field("xt", "John 1:1")],
    });

    const [note] = notesIn([cross]);
    expect(note?.isReference).toBe(true);
    expect(note?.reference).toBe("3:16");
    expect(note?.text).toBe("John 1:1");
  });

  it("leaves the note's machinery out of its text", () => {
    // `\fq` is a quotation of the word being annotated; it belongs to the
    // note but reading it as the note's text puts the lemma in front of the
    // comment.
    const withQuote = node({
      kind: "note",
      marker: "f",
      children: [field("fr", "1:5 "), field("fq", "overcome"), field("ft", "Or comprehended")],
    });

    expect(notesIn([withQuote])[0]?.text).toBe("Or comprehended");
  });

  it("numbers them in document order", () => {
    const notes = notesIn([
      node({ kind: "para", children: [footnote(10), footnote(50)] }),
      node({ kind: "para", children: [footnote(90)] }),
    ]);

    expect(notes.map((note) => note.index)).toEqual([0, 1, 2]);
    expect(notes.map((note) => note.start)).toEqual([10, 50, 90]);
  });

  it("does not descend into a note inside a note", () => {
    const nested = node({
      kind: "note",
      marker: "f",
      children: [field("ft", "outer"), node({ kind: "note", marker: "f", children: [] })],
    });

    expect(notesIn([nested])).toHaveLength(1);
  });

  it("finds the note an offset is in", () => {
    const notes = notesIn([node({ kind: "para", children: [footnote(100)] })]);
    expect(noteAt(notes, 120)).toBe(0);
    expect(noteAt(notes, 5)).toBe(-1);
  });
});

describe("converting between the two kinds", () => {
  it("turns a footnote into a cross-reference", () => {
    expect(convert("\\f + \\fr 1:5 \\ft Or comprehended\\f*")).toBe(
      "\\x + \\xo 1:5 \\xt Or comprehended\\x*",
    );
  });

  it("turns it back", () => {
    expect(convert("\\x + \\xo 1:5 \\xt John 1:1\\x*")).toBe(
      "\\f + \\fr 1:5 \\ft John 1:1\\f*",
    );
  });

  it("leaves a marker with no counterpart exactly as it was", () => {
    // Visible and fixable. Mapping `\fq` onto something approximate would be
    // the conversion quietly rewriting a translator's markup.
    const out = convert("\\f + \\fr 1:5 \\fq overcome\\fq* \\ft Or comprehended\\f*");
    expect(out).toContain("\\fq overcome\\fq*");
    expect(out).toContain("\\x + \\xo 1:5");
  });

  it("declines anything that is not a note", () => {
    expect(convert("\\v 1 In the beginning")).toBeNull();
    expect(convert("plain text")).toBeNull();
  });
});
