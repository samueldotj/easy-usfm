/**
 * The sidebar model, and the one property the rewrite has to have.
 *
 * Reading the fields is the easy half and is tested for completeness. The half
 * worth the file is the rewrite: a form that writes back into somebody's
 * Scripture must not lose, reorder or invent a line, and every test below that
 * looks like an edge case is a way that has actually gone wrong in editors
 * that shipped.
 */

import { describe, expect, it } from "vitest";

import { insertionFor } from "./insert";
import { changed, newSidebar, rewrite, sidebarsIn, wordCount } from "./sidebars";

const BOOK = [
  "\\c 1",
  "\\s1 The Beginning",
  "\\p",
  "\\v 1 In the beginning was the Word.",
  "\\v 5 The Light shines in the darkness.",
  "\\esb",
  "\\cat Word Study\\cat*",
  "\\ms The Word (Logos)",
  "\\p Greek λόγος carries both “speech” and “reason.”",
  "\\esbe",
  "\\p",
  "\\v 6 There came a man sent from God.",
].join("\n");

describe("finding sidebars", () => {
  it("reads the three fields the panel edits", () => {
    const [sidebar] = sidebarsIn(BOOK);

    expect(sidebar?.category).toBe("Word Study");
    expect(sidebar?.heading).toBe("The Word (Logos)");
    expect(sidebar?.paragraphs).toEqual([
      "Greek λόγος carries both “speech” and “reason.”",
    ]);
  });

  it("says which verse it follows", () => {
    // The one piece of context the panel shows that is not inside the block.
    expect(sidebarsIn(BOOK)[0]?.after).toBe("1:5");
  });

  it("spans exactly the block, so replacing it touches nothing else", () => {
    const [sidebar] = sidebarsIn(BOOK);
    expect(BOOK.slice(sidebar?.start, sidebar?.end)).toBe(
      [
        "\\esb",
        "\\cat Word Study\\cat*",
        "\\ms The Word (Logos)",
        "\\p Greek λόγος carries both “speech” and “reason.”",
        "\\esbe",
      ].join("\n"),
    );
  });

  it("reports line numbers the status line can show", () => {
    const [sidebar] = sidebarsIn(BOOK);
    expect([sidebar?.firstLine, sidebar?.lastLine]).toEqual([6, 10]);
  });

  it("offsets against the document when given a chapter slice", () => {
    // How the panel actually calls it: the chapter's text, and where the
    // chapter starts. Positions have to address the whole document or the
    // editor writes them into the wrong place.
    const at = BOOK.indexOf("\\c 1");
    const [sidebar] = sidebarsIn(BOOK.slice(at), at);
    expect(BOOK.slice(sidebar?.start, (sidebar?.start ?? 0) + 4)).toBe("\\esb");
  });

  it("does not offer an unclosed block", () => {
    // Half-typed, and its end is unknown. Fields for it would be an offer to
    // overwrite everything after it.
    expect(sidebarsIn("\\esb\n\\ms Half written\n\\p Still going")).toEqual([]);
  });

  it("finds several, in order", () => {
    const text = ["\\esb", "\\ms One", "\\esbe", "\\v 4 text", "\\esb", "\\ms Two", "\\esbe"].join("\n");
    expect(sidebarsIn(text).map((sidebar) => sidebar.heading)).toEqual(["One", "Two"]);
    expect(sidebarsIn(text).map((sidebar) => sidebar.index)).toEqual([0, 1]);
  });

  it("does not let a verse inside a sidebar move the reference", () => {
    // `\v` inside a study note is the note quoting a verse, not the Scripture
    // advancing. Counting it would file the next sidebar under a verse the
    // reader is nowhere near.
    const text = [
      "\\c 2",
      "\\v 3 text",
      "\\esb",
      "\\p See \\v 40 elsewhere",
      "\\esbe",
      "\\esb",
      "\\ms Second",
      "\\esbe",
    ].join("\n");
    expect(sidebarsIn(text).map((sidebar) => sidebar.after)).toEqual(["2:3", "2:3"]);
  });

  it("counts the lines it does not model", () => {
    const text = ["\\esb", "\\ms Title", "\\q1 A line of poetry", "\\p Body", "\\esbe"].join("\n");
    expect(sidebarsIn(text)[0]?.otherLines).toBe(1);
  });
});

describe("rewriting", () => {
  const only = (text: string) => {
    const sidebar = sidebarsIn(text)[0];
    if (!sidebar) throw new Error("no sidebar");
    return sidebar;
  };

  it("changes a field where it stands", () => {
    const sidebar = only(BOOK);
    const out = rewrite(sidebar, {
      category: "Theology",
      heading: "The Word",
      paragraphs: ["Shorter."],
    });

    expect(out).toBe(["\\esb", "\\cat Theology\\cat*", "\\ms The Word", "\\p Shorter.", "\\esbe"].join("\n"));
  });

  it("leaves markup it does not model exactly where it was", () => {
    // The test this module exists for. A canonical re-serialisation would put
    // the poetry after the paragraph, or lose it.
    const text = [
      "\\esb",
      "\\cat Poetry\\cat*",
      "\\q1 A line",
      "\\q2 Another",
      "\\ms Title",
      "\\p Body",
      "\\fig |src=\"x.png\"\\fig*",
      "\\esbe",
    ].join("\n");

    const out = rewrite(only(text), {
      category: "Poetry",
      heading: "New title",
      paragraphs: ["New body"],
    });

    expect(out.split("\n")).toEqual([
      "\\esb",
      "\\cat Poetry\\cat*",
      "\\q1 A line",
      "\\q2 Another",
      "\\ms New title",
      "\\p New body",
      "\\fig |src=\"x.png\"\\fig*",
      "\\esbe",
    ]);
  });

  it("adds a category the block never had, under the opening marker", () => {
    const out = rewrite(only(["\\esb", "\\ms Title", "\\p Body", "\\esbe"].join("\n")), {
      category: "Background",
      heading: "Title",
      paragraphs: ["Body"],
    });

    expect(out.split("\n")[1]).toBe("\\cat Background\\cat*");
  });

  it("removes the line when a field is cleared", () => {
    // Not `\cat \cat*`, which is a marker with no value -- a diagnostic
    // waiting to happen, produced by someone emptying a text box.
    const out = rewrite(only(BOOK), { category: null, heading: "Kept", paragraphs: ["Body"] });
    expect(out).not.toContain("\\cat");
  });

  it("adds a paragraph after the last one", () => {
    const out = rewrite(only(BOOK), {
      category: "Word Study",
      heading: "The Word (Logos)",
      paragraphs: ["First", "Second"],
    });

    expect(out.split("\n").slice(3, 5)).toEqual(["\\p First", "\\p Second"]);
  });

  it("adds the first paragraph to a block that had none", () => {
    const out = rewrite(only(["\\esb", "\\ms Title", "\\esbe"].join("\n")), {
      category: null,
      heading: "Title",
      paragraphs: ["Body"],
    });

    expect(out.split("\n")).toEqual(["\\esb", "\\ms Title", "\\p Body", "\\esbe"]);
  });

  it("drops the paragraphs the panel handed back fewer of", () => {
    const text = ["\\esb", "\\p One", "\\p Two", "\\p Three", "\\esbe"].join("\n");
    const out = rewrite(only(text), { category: null, heading: null, paragraphs: ["One"] });
    expect(out.split("\n")).toEqual(["\\esb", "\\p One", "\\esbe"]);
  });

  it("treats a second heading as content, not as the title", () => {
    const text = ["\\esb", "\\ms First", "\\p Body", "\\ms Second", "\\esbe"].join("\n");
    const out = rewrite(only(text), { category: null, heading: "Renamed", paragraphs: ["Body"] });
    expect(out.split("\n")).toEqual(["\\esb", "\\ms Renamed", "\\p Body", "\\ms Second", "\\esbe"]);
  });

  it("is a no-op when nothing was edited", () => {
    const sidebar = only(BOOK);
    const same = {
      category: sidebar.category,
      heading: sidebar.heading,
      paragraphs: [...sidebar.paragraphs],
    };

    expect(changed(sidebar, same)).toBe(false);
    expect(rewrite(sidebar, same)).toBe(BOOK.slice(sidebar.start, sidebar.end));
  });
});

describe("a new sidebar", () => {
  it("writes the markers in the order a study Bible does", () => {
    expect(
      newSidebar({ category: "Word Study", heading: "Logos", paragraphs: ["Body"] }).split("\n"),
    ).toEqual(["\\esb", "\\cat Word Study\\cat*", "\\ms Logos", "\\p Body", "\\esbe"]);
  });

  it("always has somewhere to type", () => {
    // An empty block with no `\p` gives the caret nowhere to go, so the one
    // thing a new sidebar is for cannot be done to it.
    expect(newSidebar({ category: null, heading: null, paragraphs: [] })).toContain("\\p");
  });

  it("agrees with what the Insert command writes", () => {
    // The insert command spells the block out rather than calling
    // `newSidebar`, because its caret has to land on an empty heading. This is
    // the coupling that matters: whatever it writes has to read back as a
    // sidebar, or the panel cannot edit what the toolbar just inserted.
    const inserted = insertionFor("insert-sidebar", { text: "", from: 0, to: 0 });
    const [block] = sidebarsIn(inserted?.text ?? "");

    expect(block).toBeDefined();
    expect(block?.heading).toBe("");
    expect(block?.paragraphs).toEqual([""]);
  });

  it("round-trips through the reader", () => {
    const text = newSidebar({ category: "Theology", heading: "Grace", paragraphs: ["A", "B"] });
    const [back] = sidebarsIn(text);

    expect(back?.category).toBe("Theology");
    expect(back?.heading).toBe("Grace");
    expect(back?.paragraphs).toEqual(["A", "B"]);
  });
});

describe("the word count", () => {
  it("counts across paragraphs and ignores the spacing", () => {
    expect(wordCount(["one  two", " three "])).toBe(3);
  });

  it("is zero for nothing", () => {
    expect(wordCount([])).toBe(0);
    expect(wordCount([""])).toBe(0);
  });
});
