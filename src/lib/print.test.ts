/**
 * The generated `@page` rule, and the reading of stored settings — P3.11.
 *
 * This is the one place in the interface that builds CSS as text, so the
 * interesting questions are what reaches that text: a stored file is data from
 * disk, and a number from it ends up inside a rule the browser parses.
 */

import { describe, expect, it } from "vitest";

import { defaultSize, defaults, pageRule, printClasses, restore } from "./print.svelte";

describe("paper", () => {
  it("is Letter where Letter is the paper people have", () => {
    expect(defaultSize("en-US")).toBe("letter");
    expect(defaultSize("fr-CA")).toBe("letter");
  });

  it("is A4 for the rest of the world", () => {
    expect(defaultSize("en-GB")).toBe("a4");
    expect(defaultSize("hi-IN")).toBe("a4");
    expect(defaultSize("ta")).toBe("a4");
  });

  it("reads the region, not the language", () => {
    // `en-GB` is A4 and `es-US` is Letter, so a language check gets both wrong.
    expect(defaultSize("es-US")).toBe("letter");
    expect(defaultSize("en-GB")).toBe("a4");
  });
});

describe("the generated rule", () => {
  const base = { ...defaults(), size: "a4" as const };

  it("states the page box and the margins", () => {
    const rule = pageRule({ ...base, marginOuter: 20, marginInner: 18 });

    expect(rule).toContain("size: 210mm 297mm");
    expect(rule).toContain("margin: 20mm");
    // The binding edge, which is the point of having two numbers.
    expect(rule).toContain("@page :left");
    expect(rule).toContain("margin-right: 18mm");
    expect(rule).toContain("@page :right");
    expect(rule).toContain("margin-left: 18mm");
  });

  it("carries the base size to the cascade", () => {
    expect(pageRule({ ...base, fontSize: 12.5 })).toContain("--print-font-size: 12.5pt");
  });

  it("uses the Letter box for Letter", () => {
    expect(pageRule({ ...base, size: "letter" })).toContain("size: 215.9mm 279.4mm");
  });
});

describe("reading what was stored", () => {
  it("falls back completely for anything that is not settings", () => {
    for (const stored of [null, undefined, 3, "a4", []]) {
      expect(restore(stored)).toEqual(defaults());
    }
  });

  it("keeps the good fields and replaces the bad ones", () => {
    const restored = restore({ size: "letter", fontSize: "big", notes: "chapter" });

    expect(restored.size).toBe("letter");
    expect(restored.fontSize).toBe(defaults().fontSize);
  });

  it("clamps a margin that would print a blank sheet", () => {
    // Straight into a CSS rule, so the value has to be bounded rather than
    // merely well-typed. A stored file is data, not a promise.
    expect(restore({ marginOuter: 5000 }).marginOuter).toBe(60);
    expect(restore({ marginOuter: -10 }).marginOuter).toBe(0);
    expect(restore({ fontSize: 900 }).fontSize).toBe(24);
    expect(restore({ fontSize: 0 }).fontSize).toBe(6);
  });

  it("refuses a size it does not know", () => {
    expect(restore({ size: "a4; } body { display: none" }).size).toBe(defaults().size);
  });

  it("refuses a notes placement it does not know", () => {
    expect(restore({ notes: "page" }).notes).toBe("chapter");
  });
});

describe("the classes the stylesheet reads", () => {
  it("names the notes placement", () => {
    expect(printClasses({ ...defaults(), notes: "document" })).toContain("print-notes-document");
    expect(printClasses({ ...defaults(), notes: "chapter" })).toContain("print-notes-chapter");
  });

  it("marks only what is switched off", () => {
    const all = printClasses(defaults());

    // The defaults include headings and introduction, exclude cross-references.
    expect(all).not.toContain("print-no-headings");
    expect(all).not.toContain("print-no-intro");
    expect(all).toContain("print-no-xrefs");
    expect(all).toContain("print-chapter-page");
  });

  it("marks a chapter that no longer starts a page by leaving it out", () => {
    expect(printClasses({ ...defaults(), chapterStartsPage: false })).not.toContain(
      "print-chapter-page",
    );
  });
});
