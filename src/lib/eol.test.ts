import { describe, expect, it } from "vitest";

import { LineTerminators, type Change, type Eol } from "./eol";

/** Builds terminators from a shorthand like "cl l" -> crlf, lf. */
const of = (spec: string, fallback: Eol = "lf") =>
  new LineTerminators(
    spec.split(" ").map((token) => ({ l: "lf", c: "crlf", r: "cr" })[token] as Eol),
    fallback,
  );

const change = (fromA: number, toA: number, inserted: string): Change => ({
  fromA,
  toA,
  inserted,
});

describe("serialization", () => {
  it("puts each line's own terminator back", () => {
    // A file that disagrees with itself, which is the case the whole module
    // exists for.
    expect(of("c l r").serialize("a\nb\nc\n")).toBe("a\r\nb\nc\r");
  });

  it("leaves a missing final newline missing", () => {
    expect(of("c c").serialize("a\nb\nc")).toBe("a\r\nb\r\nc");
  });

  it("handles an empty document", () => {
    expect(of("l").serialize("")).toBe("");
  });
});

describe("edits within a line", () => {
  it("changes no terminators", () => {
    const before = of("c l c");
    // Typing inside line 1, which is LF, in a file that is otherwise CRLF.
    const after = before.apply("a\nb\nc\n", [change(2, 2, "XX")]);

    expect(after.toArray()).toEqual(before.toArray());
  });

  it("survives a replacement spanning no newline", () => {
    const after = of("c l").apply("abc\ndef\n", [change(1, 2, "ZZZ")]);
    expect(after.toArray()).toEqual(["crlf", "lf"]);
  });
});

describe("splitting a line", () => {
  it("gives the new line the terminator of the line it was split from", () => {
    // The case the naive implementation gets wrong. Line 0 is CRLF in a file
    // that is mostly LF; splitting it must produce two CRLF lines, not one
    // CRLF and one LF.
    const after = of("c l l").apply("aa\nbb\ncc\n", [change(1, 1, "\n")]);

    expect(after.toArray()).toEqual(["crlf", "crlf", "lf", "lf"]);
  });

  it("splits an LF line inside a CRLF file into two LF lines", () => {
    const after = of("c l c").apply("aa\nbb\ncc\n", [change(4, 4, "\n")]);
    expect(after.toArray()).toEqual(["crlf", "lf", "lf", "crlf"]);
  });

  it("inserting several newlines inherits for each", () => {
    const after = of("r l").apply("aa\nbb\n", [change(1, 1, "\n\n\n")]);
    expect(after.toArray()).toEqual(["cr", "cr", "cr", "cr", "lf"]);
  });
});

describe("joining lines", () => {
  it("drops the terminator that was deleted", () => {
    // Deleting the newline at offset 2 joins lines 0 and 1.
    const after = of("c l r").apply("aa\nbb\ncc\n", [change(2, 3, "")]);
    expect(after.toArray()).toEqual(["lf", "cr"]);
  });

  it("keeps the surviving line's own terminator", () => {
    // Joining lines 1 and 2 leaves line 0's CRLF untouched.
    const after = of("c l r").apply("aa\nbb\ncc\n", [change(5, 6, "")]);
    expect(after.toArray()).toEqual(["crlf", "cr"]);
  });

  it("handles a deletion spanning several lines", () => {
    // Removes the terminators of lines 0, 1 and 2, leaving only line 3's.
    const after = of("c l r l").apply("aa\nbb\ncc\ndd\n", [change(1, 10, "")]);
    expect(after.toArray()).toEqual(["lf"]);
  });
});

describe("multiple changes in one transaction", () => {
  it("applies them in the coordinates of the document before the batch", () => {
    // Both offsets refer to the original document, as iterChanges reports
    // them. Applying them front-to-back without adjusting would corrupt the
    // second.
    const after = of("c l r").apply("aa\nbb\ncc\n", [
      change(1, 1, "\n"),
      change(7, 7, "\n"),
    ]);

    expect(after.toArray()).toEqual(["crlf", "crlf", "lf", "cr", "cr"]);
  });
});

describe("round trip", () => {
  it("an unedited document serializes back to its own bytes", () => {
    const original = "aa\r\nbb\ncc\rdd";
    const terminators = of("c l r");
    // The text as the editor holds it: one separator.
    const text = "aa\nbb\ncc\ndd";

    expect(terminators.serialize(text)).toBe(original);
  });

  it("an edit inside one line leaves every other line byte-identical", () => {
    const text = "aa\nbb\ncc\n";
    const after = of("c l r").apply(text, [change(4, 5, "X")]);

    expect(after.serialize("aa\nbX\ncc\n")).toBe("aa\r\nbX\ncc\r");
  });
});
