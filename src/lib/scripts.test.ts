/**
 * The script table — P1.11.
 *
 * What is tested here is the table's internal consistency and the two
 * decisions derived from it. The rendering half is a browser question and is
 * verified there; these are the parts that can be wrong silently.
 */

import { describe, expect, it } from "vitest";

import { SCRIPTS, lineHeightFor, scriptsIn, unicodeRange } from "./scripts";

const named = (name: string) => {
  const script = SCRIPTS.find((entry) => entry.name === name);
  if (!script) throw new Error(`no ${name} row`);
  return script;
};

describe("the table", () => {
  it("gives every script a sample that its own ranges cover", () => {
    // A sample outside its ranges would probe a different script's font, and
    // the missing-font notice would name the wrong one.
    for (const script of SCRIPTS) {
      const code = script.sample.codePointAt(0);
      expect(code, script.name).toBeDefined();
      expect(
        script.ranges.some(([from, to]) => code! >= from && code! <= to),
        `${script.name}: sample ${script.sample} is outside its own ranges`,
      ).toBe(true);
    }
  });

  it("gives every script a sample that is one printable character", () => {
    // A combining mark measures as zero width, so it cannot be probed — and
    // the first codepoint of a range often is one.
    for (const script of SCRIPTS) {
      expect([...script.sample], script.name).toHaveLength(1);
      expect(script.sample.trim(), script.name).not.toBe("");
    }
  });

  it("does not overlap ranges between scripts", () => {
    // Two rows claiming a codepoint means two `@font-face` rules claiming it,
    // and which wins is declaration order rather than anything considered.
    const owner = new Map<number, string>();
    for (const script of SCRIPTS) {
      for (const [from, to] of script.ranges) {
        for (let code = from; code <= to; code += 1) {
          const already = owner.get(code);
          expect(already, `U+${code.toString(16)}: ${already} and ${script.name}`).toBeUndefined();
          owner.set(code, script.name);
        }
      }
    }
  });

  it("keeps every multiplier within the range UNICODE §7 states", () => {
    // "110–120% of a Latin face's point size", or 1 for scripts that need the
    // face but not the metrics. A number outside that is a typo.
    for (const script of SCRIPTS) {
      expect(script.scale === 1 || (script.scale >= 1.1 && script.scale <= 1.2), script.name).toBe(
        true,
      );
    }
  });
});

describe("unicode-range", () => {
  it("renders as CSS expects it", () => {
    expect(unicodeRange(named("Tamil"))).toBe("U+0B80-0BFF");
    expect(unicodeRange(named("Hebrew"))).toBe("U+0590-05FF, U+FB1D-FB4F");
  });
});

describe("detecting what a document uses", () => {
  it("finds nothing in a Latin document", () => {
    expect(scriptsIn("\\id GEN\n\\c 1\n\\p\n\\v 1 In the beginning\n")).toEqual([]);
  });

  it("names the scripts that are there", () => {
    const found = scriptsIn("\\v 1 வணக்கம שלום");
    expect(found.map((script) => script.name).sort()).toEqual(["Hebrew", "Tamil"]);
  });

  it("finds a script used once in a large document", () => {
    // The case sampling got wrong. A rare script is still one the reader will
    // meet, and it is the one the missing-font notice exists for.
    const document = `\\id GEN\n\\h Genesis\n${"\\v 1 plain english text here\n".repeat(4000)}\\v 2 க\n`;
    expect(scriptsIn(document).map((script) => script.name)).toContain("Tamil");
  });

  it("is not fooled by a Latin document with punctuation", () => {
    expect(scriptsIn("\\v 1 “Quoted” — em-dashed… ‘text’\n")).toEqual([]);
  });
});

describe("line height", () => {
  it("is roomier when any script stacks marks", () => {
    // UNICODE §7: 1.7 where marks sit above and below the baseline.
    expect(lineHeightFor([named("Devanagari")])).toBe(1.7);
    expect(lineHeightFor([named("Hebrew")])).toBe(1.7);
  });

  it("is 1.5 for Latin and for scripts that do not stack", () => {
    expect(lineHeightFor([])).toBe(1.5);
    expect(lineHeightFor([named("Greek")])).toBe(1.5);
    expect(lineHeightFor([named("Han")])).toBe(1.5);
  });

  it("takes the roomier of a mixed document", () => {
    // One value for the whole document, so the question is which mistake to
    // make: Latin at 1.7 is airy, Devanagari at 1.5 has lines touching.
    expect(lineHeightFor([named("Greek"), named("Tamil")])).toBe(1.7);
  });
});
