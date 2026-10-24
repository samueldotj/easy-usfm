/**
 * The palette's two jobs: working out what was asked, and ordering the answer.
 *
 * The registry itself is also checked for the property that keeps it honest —
 * every command it offers has to be an id something actually handles.
 */

import { describe, expect, it } from "vitest";

import { PALETTE, keysFor, modeOf, rank, score } from "./commands";
import { COMMANDS } from "./insert";

describe("what the query means", () => {
  it("reads the three prefixes", () => {
    expect(modeOf(":3:16")).toEqual({ mode: "reference", term: "3:16" });
    expect(modeOf("\\esb")).toEqual({ mode: "marker", term: "esb" });
    expect(modeOf("@1043")).toEqual({ mode: "diagnostic", term: "1043" });
  });

  it("treats anything else as a command", () => {
    expect(modeOf("insert foot")).toEqual({ mode: "commands", term: "insert foot" });
    expect(modeOf("")).toEqual({ mode: "commands", term: "" });
  });

  it("only reads a prefix at the front", () => {
    // "go to 3:16" is one reference, not a mode switch halfway through.
    expect(modeOf("go to 3:16").mode).toBe("commands");
  });
});

describe("ranking", () => {
  const find = (term: string) => rank(term).map((command) => command.id);

  it("puts an exact prefix first", () => {
    expect(find("save")[0]).toBe("save");
  });

  it("finds a command by a word inside its label", () => {
    expect(find("footnote")[0]).toBe("insert-footnote");
  });

  it("finds a marker written the way the file writes it", () => {
    // Typing `esb` in the command field, rather than the marker prefix.
    expect(find("esb")[0]).toBe("insert-sidebar");
  });

  it("matches letters in order when nothing better does", () => {
    expect(find("zmin")).toContain("zoom-in");
  });

  it("ranks a label match above a match on the category alone", () => {
    // "view" is inside "Preview only" and it is also the name of the group
    // fifteen commands are in. The one that says it wins; the rest still
    // appear, which is what makes a category searchable at all.
    const viewed = find("view");
    expect(viewed.indexOf("panes-preview")).toBeLessThan(viewed.indexOf("theme:light"));
    expect(viewed).toContain("theme:light");
  });

  it("keeps the registry's order within a tie", () => {
    // Save before Save As, which is the order they are wanted in.
    const both = find("save");
    expect(both.indexOf("save")).toBeLessThan(both.indexOf("save-as"));
  });

  it("shows everything for an empty query", () => {
    expect(rank("")).toHaveLength(PALETTE.length);
  });

  it("shows nothing for a query that matches nothing", () => {
    expect(rank("qqqzzz")).toEqual([]);
  });

  it("scores no match as null", () => {
    expect(score({ id: "x", category: "File", label: "Save" }, "zzz")).toBeNull();
  });
});

describe("the registry", () => {
  it("gives every command a distinct id", () => {
    const ids = PALETTE.map((command) => command.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("offers every insert command the toolbar has", () => {
    // The palette and the toolbar are two routes to one table. A command
    // missing here is a command reachable only by knowing where the button is.
    const offered = new Set(PALETTE.map((command) => command.id));
    for (const command of COMMANDS) {
      expect(offered.has(command.id), command.id).toBe(true);
    }
  });
});

describe("shortcut spelling", () => {
  it("leaves Windows and Linux alone", () => {
    expect(keysFor("Ctrl Shift S", false)).toBe("Ctrl Shift S");
  });

  it("uses the Mac keys on a Mac", () => {
    // Printing "Ctrl S" on a Mac is worse than printing nothing: it is a key
    // that does not work, presented as one that does.
    expect(keysFor("Ctrl Shift S", true)).toBe("⌘ ⇧ S");
  });

  it("has nothing to say about a command with no shortcut", () => {
    expect(keysFor(undefined, true)).toBeUndefined();
  });
});
