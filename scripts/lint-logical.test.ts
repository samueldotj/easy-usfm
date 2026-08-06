/**
 * The lint catches things — P1.12 and P3.1.
 *
 * Kept beside the script rather than under `src/`, because the raw-markup
 * cases have to contain the very strings the lint bans and a test file inside
 * the scanned tree would trip on its own fixtures. Exempting them was the
 * alternative and is worse: SECURITY 1 says `{@html}` appears *nowhere*, and a
 * ban with an escape hatch is a convention again.
 *
 * The whole point of these: the lint passed on the first run, against the real
 * tree, which is exactly the situation where "it passes" is no evidence at
 * all. A check that cannot fail is a check nobody has.
 */

import { describe, expect, it } from "vitest";

// @ts-expect-error -- a plain ESM script, deliberately not part of the app's
// TypeScript program. It is build tooling, and giving it types would mean
// compiling it, which is more machinery than a 200-line checker deserves.
import { check, checkMarkup } from "./lint-logical.mjs";

const found = (source: string, isCss = true): string[] =>
  (check(source, isCss) as { found: string }[]).map((problem) => problem.found);

const markup = (source: string): string[] =>
  (checkMarkup(source) as { found: string }[]).map((problem) => problem.found);

describe("banning raw markup", () => {
  it("catches Svelte's raw directive", () => {
    expect(markup("{@html node.text}")).toEqual(["{@html}"]);
  });

  it("catches the DOM routes to the same place", () => {
    // Reaching for innerHTML gets there without tripping the first rule.
    expect(markup("element.innerHTML = value;")).toEqual([".innerHTML ="]);
    expect(markup("element.outerHTML = value;")).toEqual([".outerHTML ="]);
    expect(markup('el.insertAdjacentHTML("beforeend", s);')).toEqual(["insertAdjacentHTML()"]);
  });

  it("does not fire on reading innerHTML, which creates no markup", () => {
    expect(markup("const length = element.innerHTML.length;")).toEqual([]);
  });

  it("does not fire on ordinary text interpolation", () => {
    expect(markup("{node.text}")).toEqual([]);
    expect(markup("<span>{value}</span>")).toEqual([]);
  });

  it("has no escape hatch", () => {
    // Deliberate. SECURITY 1's control is that no path exists, and a marker
    // that waves one through would make it a convention again.
    expect(markup("{@html x} /* lint-logical-ok: no */")).toEqual(["{@html}"]);
  });
});

describe("catching physical properties", () => {
  it("flags the directional ones", () => {
    expect(found(".a { margin-left: 1rem; }")).toEqual(["margin-left"]);
    expect(found(".a { padding-right: 1rem; }")).toEqual(["padding-right"]);
    expect(found(".a { border-top: 1px solid red; }")).toEqual(["border-top"]);
  });

  it("flags sizes, which are the ones that look innocent", () => {
    expect(found(".a { width: 100%; }")).toEqual(["width"]);
    expect(found(".a { max-height: 4rem; }")).toEqual(["max-height"]);
  });

  it("flags directional values under non-directional properties", () => {
    // `text-align: left` is the same mistake as `margin-left`, and the one
    // that survives review because the property name looks fine.
    expect(found(".a { text-align: left; }")).toEqual(["text-align: left"]);
    expect(found(".a { float: right; }")).toEqual(["float: right"]);
  });

  it("finds more than one on a line", () => {
    expect(found(".a { margin-left: 0; width: 2rem; }").sort()).toEqual(["margin-left", "width"]);
  });
});

describe("not crying wolf", () => {
  it("passes the logical spellings", () => {
    expect(
      found(`.a {
        margin-inline-start: 1rem;
        padding-block-end: 2rem;
        inline-size: 100%;
        min-block-size: 0;
        border-inline-end: 1px solid red;
        text-align: start;
        inset-inline-start: 0;
      }`),
    ).toEqual([]);
  });

  it("does not match a logical property that ends in a physical name", () => {
    // The bug an unanchored search has: `border-inline-start-width` contains
    // "width", and `inset-block-start` contains "top" nowhere but its
    // neighbours do. Anchoring at the declaration start is what prevents it.
    expect(found(".a { border-inline-start-width: 1px; }")).toEqual([]);
    expect(found(".a { max-inline-size: 100%; }")).toEqual([]);
  });

  it("ignores anything outside a style block in a Svelte file", () => {
    // Markup and script are full of `width=` attributes and `height`
    // variables, none of which is CSS.
    const component = `<script lang="ts">
      let width = 10;
      const height = compute({ left: 1, right: 2 });
    </script>

    <img src="x.png" width="10" height="10" alt="" />
    <div style="ignored">text-align: left</div>

    <style>
      .a { inline-size: 100%; }
    </style>`;

    expect(found(component, false)).toEqual([]);
  });

  it("honours an exemption that gives a reason", () => {
    expect(found(".a { width: 1px; /* lint-logical-ok: it is a hairline */ }")).toEqual([]);
  });

  it("does not read a commented-out property as code", () => {
    expect(found(".a { /* margin-left: 1rem; */ inline-size: 0; }")).toEqual([]);
  });
});
