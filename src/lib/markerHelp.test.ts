/**
 * The reference page's two derivations.
 *
 * The examples are generated from the marker table, so the test is that they
 * follow the table's own facts — a marker that closes shows its closing, one
 * that takes a number shows a number. The descriptions are matched by family,
 * so the test is that the longest family wins: `\iot` is an outline *title*,
 * not an outline entry, and a shorter prefix claiming it would put a confident
 * wrong sentence on the page.
 */

import { describe, expect, it } from "vitest";

import { exampleFor, helpFor, matches, type MarkerRow } from "./markerHelp";

function row(marker: string, overrides: Partial<MarkerRow> = {}): MarkerRow {
  return {
    marker,
    class: "paragraph",
    closing: "none",
    nests_under: [],
    attributes: [],
    default_attr: null,
    since: null,
    deprecated_in: null,
    replacement: null,
    publishable: true,
    ...overrides,
  };
}

describe("examples", () => {
  it("close a character marker that closes", () => {
    const example = exampleFor(row("bd", { class: "character", closing: "explicit" }));
    expect(example).toBe("\\bd text\\bd*");
  });

  it("do not close one that does not", () => {
    const example = exampleFor(row("qs", { class: "character", closing: "none" }));
    expect(example).toBe("\\qs text");
  });

  it("show a milestone as a position, not a span", () => {
    expect(exampleFor(row("ts-s", { class: "milestone" }))).toBe("\\ts-s\\*");
  });

  it("show a note with its caller", () => {
    expect(exampleFor(row("f", { class: "note" }))).toContain("\\f + ");
    expect(exampleFor(row("f", { class: "note" }))).toContain("\\f*");
  });

  it("show a number for the markers that take one", () => {
    expect(exampleFor(row("c"))).toBe("\\c 1");
    expect(exampleFor(row("v"))).toBe("\\v 1");
  });

  it("show nothing after the markers that take nothing", () => {
    expect(exampleFor(row("b"))).toBe("\\b");
    expect(exampleFor(row("pb"))).toBe("\\pb");
  });

  it("show the attributes a marker accepts", () => {
    const example = exampleFor(
      row("fig", { class: "paragraph", attributes: ["alt", "src", "size"] }),
    );
    expect(example).toContain('alt="…"');
    expect(example).toContain('src="…"');
  });
});

describe("descriptions", () => {
  it("describe a numbered marker by its family", () => {
    // The level is in the syntax, not in the meaning.
    expect(helpFor(row("q1")).description).toBe(helpFor(row("q")).description);
    expect(helpFor(row("q3")).family).toBe("q");
  });

  it("let the longer family win", () => {
    // `\iot` is the title above an outline, not an entry in one. A shorter
    // prefix claiming it would put a confident wrong sentence on the page.
    expect(helpFor(row("iot")).family).toBe("iot");
    expect(helpFor(row("io2")).family).toBe("io");
    expect(helpFor(row("iot")).description).not.toBe(helpFor(row("io2")).description);
  });

  it("describe a milestone by its base, not its half", () => {
    expect(helpFor(row("qt-s", { class: "milestone" })).family).toBe("qt");
  });

  it("say nothing rather than guess", () => {
    // Not a `z`-prefixed name: USFM reserves that prefix for extensions, so
    // `\zsomething` is genuinely describable and is described.
    const unknown = helpFor(row("uuu"));
    expect(unknown.description).toBeNull();
    expect(unknown.family).toBeNull();
    // The syntax is still there, because that part is derived rather than
    // written and is true whatever the marker means.
    expect(unknown.example.length).toBeGreaterThan(0);
  });
});

describe("search", () => {
  const help = helpFor(row("f", { class: "note" }));

  it("finds a marker by name, with or without the backslash", () => {
    expect(matches(help, "f")).toBe(true);
    expect(matches(help, "\\f")).toBe(true);
  });

  it("finds it by what it does", () => {
    // Someone looking for "footnote" does not know it is spelled `\f`.
    expect(matches(help, "footnote")).toBe(true);
  });

  it("finds it by class", () => {
    expect(matches(help, "note")).toBe(true);
  });

  it("shows everything for an empty query", () => {
    expect(matches(help, "   ")).toBe(true);
  });

  it("excludes what does not match", () => {
    expect(matches(help, "tablecloth")).toBe(false);
  });
});
