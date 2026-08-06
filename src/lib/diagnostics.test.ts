/**
 * The behaviour worth testing here is where diagnostics *land*.
 *
 * P2.7's acceptance criterion is that they land on the right characters in
 * mixed-script text, and that is exactly the thing that passes on every
 * fixture written in English. So the fixtures are not written in English.
 */

import { EditorSelection, EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";

import {
  diagnostics as extension,
  diagnosticRange,
  nextDiagnostic,
  setDiagnostics,
  stepSpec,
  type Located,
} from "./diagnostics";
import type { Diagnostic } from "../worker/protocol";

function diagnostic(start: number, end: number, over: Partial<Diagnostic> = {}): Diagnostic {
  return {
    code: "USFM-E001",
    severity: "error",
    start,
    end,
    line: 1,
    message: "something is wrong",
    ...over,
  };
}

function stateOf(
  doc: string,
  list: Diagnostic[],
  selection?: number | { from: number; to: number },
): EditorState {
  const state = EditorState.create({
    doc,
    extensions: [extension],
    selection:
      selection === undefined
        ? undefined
        : typeof selection === "number"
          ? EditorSelection.cursor(selection)
          : EditorSelection.range(selection.from, selection.to),
  });
  return state.update({ effects: setDiagnostics.of(list) }).state;
}

/** Every underline currently placed, with the text it covers. */
function underlined(state: EditorState): { from: number; to: number; text: string }[] {
  const found: { from: number; to: number; text: string }[] = [];
  let index = 0;
  // Walked through the public queries, since that is what the editor uses.
  for (let range = diagnosticRange(state, index); range; range = diagnosticRange(state, ++index)) {
    found.push({
      from: range.from,
      to: range.to,
      text: state.doc.sliceString(range.from, range.to),
    });
  }
  return found;
}

describe("placing diagnostics", () => {
  it("lands on the named characters when the line is not ASCII", () => {
    // Tamil: every character before the target is one UTF-16 unit and three
    // UTF-8 bytes. A byte offset that leaked through would point far past it.
    const doc = "\\v 1 க்ஷேமம் \\bd உலகம்";
    const marker = doc.indexOf("\\bd");

    expect(underlined(stateOf(doc, [diagnostic(marker, marker + 3)]))).toEqual([
      { from: marker, to: marker + 3, text: "\\bd" },
    ]);
  });

  it("lands on the named characters when the line is right-to-left", () => {
    const doc = "\\v 1 שלום \\nd עולם\\nd*";
    const word = doc.indexOf("עולם");

    expect(underlined(stateOf(doc, [diagnostic(word, word + 4)]))).toEqual([
      { from: word, to: word + 4, text: "עולם" },
    ]);
  });

  it("counts an astral character as the two units the editor counts it as", () => {
    // U+1D400 is one character, two UTF-16 units, four UTF-8 bytes. The three
    // spaces disagree here more than anywhere else.
    const doc = "\\v 1 \u{1D400} end";
    const end = doc.indexOf("end");

    expect(underlined(stateOf(doc, [diagnostic(end, end + 3)]))).toEqual([
      { from: end, to: end + 3, text: "end" },
    ]);
  });

  it("widens an empty diagnostic by a whole grapheme cluster", () => {
    // Something absent rather than wrong reports a zero-width span, and
    // CodeMirror refuses an empty mark. Widening by one UTF-16 unit here would
    // underline half of க் -- a base and its virama, one cluster.
    const doc = "க்ஷேமம்";
    const placed = underlined(stateOf(doc, [diagnostic(0, 0)]));

    expect(placed).toEqual([{ from: 0, to: 2, text: "க்" }]);
  });

  it("widens backwards when there is nothing ahead of it on the line", () => {
    const doc = "\\v 1 abc";
    const placed = underlined(stateOf(doc, [diagnostic(doc.length, doc.length)]));

    expect(placed).toEqual([{ from: doc.length - 1, to: doc.length, text: "c" }]);
  });

  it("has nothing to underline on an empty line, and does not crash", () => {
    expect(underlined(stateOf("", [diagnostic(0, 0)]))).toEqual([]);
  });

  it("ignores a diagnostic describing text that has since been deleted", () => {
    expect(underlined(stateOf("short", [diagnostic(400, 410)]))).toEqual([]);
  });

  it("accepts diagnostics the engine did not emit in document order", () => {
    // The engine reports them as it finds them, and more than one pass
    // contributes. A range set built out of order throws.
    const doc = "\\v 1 one two three";
    const state = stateOf(doc, [diagnostic(13, 18), diagnostic(5, 8), diagnostic(9, 12)]);

    expect(underlined(state).map((found) => found.text).sort()).toEqual(["one", "three", "two"]);
  });
});

describe("moving with the document", () => {
  it("keeps a diagnostic on its word when text is inserted before it", () => {
    const doc = "\\v 1 க்ஷேமம் bad";
    const bad = doc.indexOf("bad");
    const before = stateOf(doc, [diagnostic(bad, bad + 3)]);

    // Typing four characters near the start, as would happen between one parse
    // and the next arriving from the worker.
    const after = before.update({ changes: { from: 5, insert: "மொழி" } }).state;

    expect(underlined(after)).toEqual([{ from: bad + 4, to: bad + 7, text: "bad" }]);
  });

  it("drops a diagnostic whose text is deleted outright", () => {
    const doc = "\\v 1 bad";
    const before = stateOf(doc, [diagnostic(5, 8)]);
    const after = before.update({ changes: { from: 5, to: 8 } }).state;

    expect(underlined(after)).toEqual([]);
  });
});

/** Where it landed, without the walk position — that is asserted separately. */
function where(found: Located | null): { from: number; to: number } | null {
  return found && { from: found.from, to: found.to };
}

describe("F8", () => {
  const doc = "\\v 1 one two three";
  const list = [diagnostic(5, 8), diagnostic(9, 12), diagnostic(13, 18)];

  it("finds the next one strictly after the cursor", () => {
    expect(where(nextDiagnostic(stateOf(doc, list, 0), true))).toEqual({ from: 5, to: 8 });
    // On the first one already: pressing again must move on, not stay.
    expect(where(nextDiagnostic(stateOf(doc, list, 5), true))).toEqual({ from: 9, to: 12 });
  });

  it("wraps rather than stopping at the end", () => {
    expect(where(nextDiagnostic(stateOf(doc, list, 18), true))).toEqual({ from: 5, to: 8 });
    expect(where(nextDiagnostic(stateOf(doc, list, 0), false))).toEqual({ from: 13, to: 18 });
  });

  it("goes backwards", () => {
    expect(where(nextDiagnostic(stateOf(doc, list, 13), false))).toEqual({ from: 9, to: 12 });
  });

  it("steps out of the diagnostic the cursor is inside, in both directions", () => {
    // Inside the third. Comparing both directions against the start would
    // return this same diagnostic going backwards, forever.
    expect(where(nextDiagnostic(stateOf(doc, list, 14), false))).toEqual({ from: 9, to: 12 });
    expect(where(nextDiagnostic(stateOf(doc, list, 14), true))).toEqual({ from: 5, to: 8 });
  });

  it("returns to where it came from", () => {
    // F8 selects the whole run, leaving the head at its end. Shift+F8 from
    // there has to be the one before, not the one just landed on -- which is
    // what makes the two keys a round trip rather than two ways to get stuck.
    const first = where(nextDiagnostic(stateOf(doc, list, 0), true))!;
    const second = where(nextDiagnostic(stateOf(doc, list, first.to), true))!;

    expect(second).toEqual({ from: 9, to: 12 });
    expect(where(nextDiagnostic(stateOf(doc, list, second.to), false))).toEqual(first);
  });

  it("has nowhere to go in a clean document", () => {
    expect(where(nextDiagnostic(stateOf(doc, [], 0), true))).toBeNull();
  });

  it("visits diagnostics that are nested inside another one", () => {
    // The shape real USFM produces: an unclosed \bd spans everything after it,
    // and the diagnostics for the markers inside fall within that span. Asking
    // only for the next one *starting* after the cursor skips every one of
    // them, which on a genuinely broken file is most of the list.
    const outer = diagnostic(0, 18);
    const list = [outer, diagnostic(5, 8), diagnostic(9, 12), diagnostic(13, 18)];

    // Standing on the outer one, as F8 leaves you.
    let at = { from: 0, to: 18 };
    const visited = [];
    for (let step = 0; step < 4; step += 1) {
      at = where(nextDiagnostic(stateOf(doc, list, at), true))!;
      visited.push(at);
    }

    expect(visited).toEqual([
      { from: 5, to: 8 },
      { from: 9, to: 12 },
      { from: 13, to: 18 },
      { from: 0, to: 18 },
    ]);
  });

  it("walks past two diagnostics covering exactly the same run", () => {
    // Routine in real USFM: an unclosed \bd is both "missing its close" and
    // "spanning a verse boundary", reported over the same characters. Matching
    // the selection against the list finds the first of them every time, so
    // without the remembered position F8 alternates between two
    // indistinguishable places and never reaches the third diagnostic.
    const list = [diagnostic(5, 8), diagnostic(5, 8), diagnostic(9, 12)];
    let state = stateOf(doc, list, 0);
    const seen = [];

    for (let step = 0; step < 4; step += 1) {
      state = state.update(stepSpec(state, true)!).state;
      seen.push({ from: state.selection.main.from, to: state.selection.main.to });
    }

    expect(seen).toEqual([
      { from: 5, to: 8 },
      { from: 5, to: 8 }, // the duplicate
      { from: 9, to: 12 }, // reached only because the two were told apart
      { from: 5, to: 8 }, // wrapped
    ]);
  });

  it("forgets where the walk was once the user moves the cursor", () => {
    const list = [diagnostic(5, 8), diagnostic(9, 12), diagnostic(13, 18)];
    let state = stateOf(doc, list, 0);
    state = state.update(stepSpec(state, true)!).state;

    // A click, somewhere else entirely.
    state = state.update({ selection: { anchor: 17 } }).state;

    expect(where(nextDiagnostic(state, true))).toEqual({ from: 5, to: 8 });
  });

  it("walks back out of a nest the way it came in", () => {
    const list = [diagnostic(0, 18), diagnostic(5, 8), diagnostic(9, 12)];

    expect(where(nextDiagnostic(stateOf(doc, list, { from: 9, to: 12 }), false))).toEqual({
      from: 5,
      to: 8,
    });
    expect(where(nextDiagnostic(stateOf(doc, list, { from: 5, to: 8 }), false))).toEqual({
      from: 0,
      to: 18,
    });
  });
});
