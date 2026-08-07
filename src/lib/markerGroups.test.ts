/**
 * The grouping, and the one property that matters about it.
 *
 * Groups are written by hand, because `markers.toml` records what a marker *is*
 * and never which markers collaborate. Hand-written means a marker can be
 * missed — so the test that earns its keep is that nothing is lost: every
 * marker the engine knows lands in exactly one group, and the catch-all makes
 * that true even for one nobody has placed.
 */

import { describe, expect, it } from "vitest";

import { GROUPS, groupOf, grouped } from "./markerGroups";
import { helpFor, type MarkerRow } from "./markerHelp";

function row(marker: string): MarkerRow {
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
  };
}

describe("placing markers", () => {
  it("keeps a table's markers together", () => {
    // The example the reference exists for: five markers that mean nothing
    // apart, filed a long way from each other by an alphabetical list.
    for (const marker of ["tr", "th1", "th2", "thr3", "tc1", "tcr2"]) {
      expect(groupOf(marker)?.id, marker).toBe("tables");
    }
  });

  it("keeps a paired marker with its partner", () => {
    expect(groupOf("erq")?.id).toBe(groupOf("erqe")?.id);
    expect(groupOf("esb")?.id).toBe(groupOf("esbe")?.id);
    expect(groupOf("qt-s")?.id).toBe(groupOf("qt-e")?.id);
  });

  it("keeps a note's inner markers with the note", () => {
    for (const marker of ["f", "fr", "ft", "fq", "fqa", "fk"]) {
      expect(groupOf(marker)?.id, marker).toBe("footnotes");
    }
    for (const marker of ["x", "xo", "xt"]) {
      expect(groupOf(marker)?.id, marker).toBe("cross-references");
    }
  });

  it("lets the longer prefix win", () => {
    // `\iot` is the outline's title and `\io1` an entry in it; both are
    // introduction, but the rule that places them has to be longest-first or
    // `\thr` would be filed by `th` — right answer, wrong reason.
    expect(groupOf("iot")?.id).toBe("introduction");
    expect(groupOf("thr2")?.id).toBe("tables");
  });

  it("puts every level of a numbered marker in one place", () => {
    for (const level of [1, 2, 3, 4]) {
      expect(groupOf(`q${level}`)?.id, `q${level}`).toBe("poetry");
      expect(groupOf(`li${level}`)?.id, `li${level}`).toBe("lists");
    }
  });
});

describe("nothing is lost", () => {
  it("gives every marker exactly one group", () => {
    const table = ["tr", "th1", "f", "ft", "q1", "bd", "zzz-unplaced"].map((m) => helpFor(row(m)));
    const groups = grouped(table);

    const placed = groups.flatMap((group) => group.markers.map((help) => help.marker));
    expect(placed.sort()).toEqual(table.map((help) => help.marker).sort());
    expect(new Set(placed).size).toBe(placed.length);
  });

  it("shows no catch-all when everything is placed", () => {
    const table = ["tr", "f", "q1"].map((m) => helpFor(row(m)));
    expect(grouped(table).some((group) => group.id === "other")).toBe(false);
  });

  it("catches a marker nobody has placed rather than dropping it", () => {
    // Visible and harmless, which is the right failure for a hand-written
    // grouping — the alternative is guessing, and guessing files things wrongly
    // without saying so.
    const groups = grouped([helpFor(row("qqunplaced"))]);
    expect(groups.at(-1)?.id).toBe("other");
  });
});

describe("the groups themselves", () => {
  it("give every group a combined example using its own markers", () => {
    for (const group of GROUPS) {
      expect(group.example.length, group.id).toBeGreaterThan(0);
      // The example has to use markers from the group it illustrates.
      const uses = group.prefixes.some((prefix) => group.example.includes(`\\${prefix}`));
      expect(uses, `${group.id} example should use its own markers`).toBe(true);
    }
  });

  it("give every group a distinct id", () => {
    const ids = GROUPS.map((group) => group.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
