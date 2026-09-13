import { describe, expect, it } from "vitest";

import { identityOf } from "./identity";

const HEADER = [
  "\\id JHN - Berean Standard Bible",
  "\\usfm 3.0",
  "\\h John",
  "\\toc1 The Gospel according to John",
  "\\toc2 John",
  "\\mt1 John",
  "\\c 1",
].join("\n");

describe("reading a file's identity", () => {
  it("takes the book from the running header", () => {
    expect(identityOf(HEADER).book).toBe("John");
  });

  it("takes the translation from what follows the book code", () => {
    expect(identityOf(HEADER).translation).toBe("Berean Standard Bible");
    expect(identityOf(HEADER).code).toBe("JHN");
  });

  it("accepts an id with no description", () => {
    const identity = identityOf("\\id GEN\n\\h Genesis");
    expect(identity.code).toBe("GEN");
    expect(identity.translation).toBeNull();
    expect(identity.book).toBe("Genesis");
  });

  it("falls back through the names a file might carry", () => {
    // A file with no `\h` still has a name a reader would recognise.
    expect(identityOf("\\id MRK\n\\toc2 Mark").book).toBe("Mark");
    expect(identityOf("\\id MRK\n\\mt1 The Gospel of Mark").book).toBe("The Gospel of Mark");
  });

  it("prefers the running header over the longer forms", () => {
    // `\toc1` is the long form for a contents page; the bar has one line.
    expect(identityOf("\\id JHN\n\\toc1 The Gospel according to John\n\\h John").book).toBe("John");
  });

  it("says nothing about a file that says nothing", () => {
    expect(identityOf("")).toEqual({ book: null, translation: null, code: null });
    expect(identityOf("\\p Just some text").book).toBeNull();
  });

  it("does not read the body", () => {
    // An `\h` sixty lines in is not a header; treating it as one would put a
    // stray line of Scripture in the title bar.
    const long = ["\\id JHN"].concat(Array(80).fill("\\p text"), ["\\h Not a header"]).join("\n");
    expect(identityOf(long).book).toBeNull();
  });

  it("survives a file with no newline at all", () => {
    expect(identityOf("\\id JHN - BSB").translation).toBe("BSB");
  });
});
