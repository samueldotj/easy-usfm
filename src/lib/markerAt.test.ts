import { describe, expect, it } from "vitest";

import { markerAt } from "./markerAt";

/** The caret is written as `|`, which is how a reader sees it. */
function at(source: string): string | null {
  const offset = source.indexOf("|");
  return markerAt(source.replace("|", ""), offset);
}

describe("the marker at the caret", () => {
  it("is the paragraph marker when nothing else is open", () => {
    expect(at("\\p In the beginning| was the Word")).toBe("p");
    expect(at("\\q1 A line of| poetry")).toBe("q1");
  });

  it("is the character marker when the caret is inside one", () => {
    expect(at("\\p The \\bd word| here\\bd*")).toBe("bd");
  });

  it("goes back to the paragraph once the span has closed", () => {
    expect(at("\\p The \\bd word\\bd* and| the rest")).toBe("p");
  });

  it("is the innermost of several open at once", () => {
    expect(at("\\p a \\bd b \\+nd c| \\+nd*\\bd*")).toBe("nd");
  });

  it("drops the plus that marks nesting", () => {
    // `\+nd` is `\nd` written inside another character marker. The plus is
    // syntax, and looking it up as a marker name finds nothing.
    expect(at("\\p \\bd x \\+nd y|")).toBe("nd");
  });

  it("is the marker being typed", () => {
    expect(at("\\p text \\bd|")).toBe("bd");
    expect(at("\\es|")).toBe("es");
  });

  it("looks at this line only", () => {
    // A `\bd` left open on the line above is a mistake in the file, not a span
    // the caret is inside -- character markers do not cross a paragraph.
    expect(at("\\p \\bd unclosed\n\\p next line|")).toBe("p");
  });

  it("has nothing to say about a line with no markers", () => {
    expect(at("just some text|")).toBeNull();
    expect(at("|")).toBeNull();
  });

  it("clamps an offset past the end", () => {
    expect(markerAt("\\p text", 9999)).toBe("p");
    expect(markerAt("\\p text", -5)).toBeNull();
  });
});
