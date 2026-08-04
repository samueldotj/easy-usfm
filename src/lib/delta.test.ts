import { describe, expect, it } from "vitest";

import { checksum } from "./checksum";
import { applyToMirror, CHECKSUM_INTERVAL, DeltaBuffer, type Batch } from "./delta";
import type { Edit } from "../worker/protocol";

/** A tiny stand-in for the editor: the authoritative text. */
class Editor {
  text = "";

  /** Applies an edit and returns it in the form the protocol carries. */
  apply(from: number, to: number, insert: string): Edit {
    this.text = this.text.slice(0, from) + insert + this.text.slice(to);
    return { from, to, insert };
  }
}

describe("batching", () => {
  it("sends one batch per transaction", () => {
    const buffer = new DeltaBuffer();
    const editor = new Editor();
    const edit = editor.apply(0, 0, "\\id GEN");

    const batch = buffer.push([edit], editor.text);
    expect(batch?.edits).toEqual([edit]);
  });

  it("sends nothing for an empty transaction", () => {
    expect(new DeltaBuffer().push([], "")).toBeNull();
  });

  it("includes a checksum every CHECKSUM_INTERVAL batches", () => {
    const buffer = new DeltaBuffer();
    const editor = new Editor();
    const withChecksum: number[] = [];

    for (let index = 0; index < CHECKSUM_INTERVAL * 2; index += 1) {
      const edit = editor.apply(editor.text.length, editor.text.length, "x");
      const batch = buffer.push([edit], editor.text);
      if (batch?.checksum !== undefined) withChecksum.push(index);
    }

    // Every 50th, and the value describes the document at that moment.
    expect(withChecksum).toEqual([CHECKSUM_INTERVAL - 1, CHECKSUM_INTERVAL * 2 - 1]);
  });

  it("checksums at an idle boundary even with nothing to send", () => {
    // Drift can come from a dropped message rather than from an edit, so a
    // quiet moment is still worth verifying.
    const batch = new DeltaBuffer().idle("\\id GEN\n");

    expect(batch?.edits).toEqual([]);
    expect(batch?.checksum).toBe(checksum("\\id GEN\n"));
  });
});

describe("composition suppression", () => {
  /**
   * A synthetic input-method sequence: the intermediate states a
   * transliteration keyboard produces before committing a Tamil syllable.
   *
   * UNICODE §5 — the mirror must see the committed text and never the
   * syllables it was assembled from.
   */
  const SEQUENCE = ["க", "க்", "க்ஷ", "க்ஷே"];

  it("emits exactly one batch for a composed word", () => {
    const buffer = new DeltaBuffer();
    const editor = new Editor();
    const sent: Batch[] = [];

    buffer.startComposition();
    for (const stage of SEQUENCE) {
      const edit = editor.apply(0, editor.text.length, stage);
      const batch = buffer.push([edit], editor.text);
      if (batch) sent.push(batch);
    }

    // Nothing at all while composing. Sending a provisional state is what
    // makes the preview flicker through partial syllables.
    expect(sent).toEqual([]);
    expect(buffer.composing).toBe(true);
    expect(buffer.pendingCount).toBe(SEQUENCE.length);

    const committed = buffer.endComposition(editor.text);
    expect(committed).not.toBeNull();
    sent.push(committed!);

    expect(sent).toHaveLength(1);
  });

  it("the mirror reconstructs the committed text exactly", () => {
    const buffer = new DeltaBuffer();
    const editor = new Editor();
    let mirror = "";

    editor.text = "\\v 1 ";
    mirror = "\\v 1 ";

    buffer.startComposition();
    for (const stage of SEQUENCE) {
      buffer.push([editor.apply(5, editor.text.length, stage)], editor.text);
    }
    const batch = buffer.endComposition(editor.text)!;
    mirror = applyToMirror(mirror, batch);

    expect(mirror).toBe(editor.text);
    expect(mirror).toBe("\\v 1 க்ஷே");
  });

  it("a composition producing nothing sends nothing", () => {
    const buffer = new DeltaBuffer();
    buffer.startComposition();
    expect(buffer.endComposition("")).toBeNull();
  });

  it("resumes ordinary batching after the composition ends", () => {
    const buffer = new DeltaBuffer();
    const editor = new Editor();

    buffer.startComposition();
    buffer.push([editor.apply(0, 0, "க")], editor.text);
    buffer.endComposition(editor.text);

    const after = buffer.push([editor.apply(1, 1, "!")], editor.text);
    expect(after?.edits).toHaveLength(1);
    expect(buffer.composing).toBe(false);
  });
});

describe("the mirror after a long session", () => {
  /**
   * ARCHITECTURE §9's acceptance: the mirror matches the editor after a
   * scripted 10,000-edit session.
   *
   * Deterministic rather than random — a mirror bug that reproduces only on
   * one seed is a mirror bug nobody can fix. The alphabet is the awkward one:
   * conjuncts, astral characters, and a combining mark, so an implementation
   * that counts code points instead of UTF-16 units fails here rather than in
   * front of a translator.
   */
  it("matches the editor exactly, and agrees on the checksum", () => {
    const alphabet = ["a", " ", "\n", "\\v 1 ", "க்ஷ", "\u{1D400}", "e\u{301}", "שלום"];
    const buffer = new DeltaBuffer();
    const editor = new Editor();
    let mirror = "";
    let seed = 12345;

    // A small deterministic PRNG, so a failure is reproducible by rerunning.
    const next = (bound: number) => {
      seed = (Math.imul(seed, 1103515245) + 12345) & 0x7fffffff;
      return seed % bound;
    };

    for (let index = 0; index < 10_000; index += 1) {
      const length = editor.text.length;
      const from = length === 0 ? 0 : next(length + 1);

      // A mix of insertions, replacements, and deletions.
      const kind = next(10);
      let to = from;
      let insert = "";

      if (kind < 6) {
        insert = alphabet[next(alphabet.length)]!;
      } else if (kind < 8 && from < length) {
        to = Math.min(length, from + 1 + next(4));
      } else if (from < length) {
        to = Math.min(length, from + 1 + next(3));
        insert = alphabet[next(alphabet.length)]!;
      } else {
        insert = alphabet[next(alphabet.length)]!;
      }

      const batch = buffer.push([editor.apply(from, to, insert)], editor.text);
      if (batch) mirror = applyToMirror(mirror, batch);

      // Every checksum the protocol sends must agree with the mirror, at the
      // moment it is sent. Checking only at the end would let the two drift
      // apart and back together unnoticed.
      if (batch?.checksum !== undefined) {
        expect(checksum(mirror)).toBe(batch.checksum);
      }
    }

    // Anything the buffer is still holding.
    const final = buffer.idle(editor.text);
    if (final) mirror = applyToMirror(mirror, final);

    expect(mirror).toBe(editor.text);
    expect(checksum(mirror)).toBe(checksum(editor.text));
    expect(editor.text.length).toBeGreaterThan(0);
  });
});
