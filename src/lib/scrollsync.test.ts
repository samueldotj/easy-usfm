/**
 * The parts of the scroll sync worth testing are its edges.
 *
 * Both directions work on any document long enough to scroll, and that is the
 * case that passes without being written down. What broke in use was the ends:
 * the top of the document, where the header carries no block of its own, and
 * the bottom, where the last paragraph has scrolled past the top edge. Both
 * answered "nothing", and "nothing" leaves the other pane where it was — a sync
 * that silently stops working exactly where the user notices.
 *
 * The driver rule gets tests of its own because it is the thing standing
 * between two panes and an infinite loop.
 */

import { beforeEach, describe, expect, it } from "vitest";

import { ScrollSync, elementFor, topmostOffset } from "./scrollsync";

/**
 * A container whose blocks have positions, without a layout engine.
 *
 * Hand-built rather than through jsdom, which would be a dependency added to
 * answer three questions about geometry. These functions ask the DOM only for
 * `querySelectorAll`, `getBoundingClientRect` and `dataset`, so that is what is
 * here — the arithmetic under test is the real thing either way.
 */
function container(
  blocks: { start: number; top: number; height: number }[],
  viewportTop = 100,
): HTMLElement {
  const children = blocks.map((block) => ({
    dataset: { start: String(block.start) },
    getBoundingClientRect: () => rect(block.top, block.height),
  }));

  return {
    getBoundingClientRect: () => rect(viewportTop, 500),
    querySelectorAll: () => children,
  } as unknown as HTMLElement;
}

function rect(top: number, height: number): DOMRect {
  return { top, height, bottom: top + height, left: 0, right: 0, width: 0, x: 0, y: top } as DOMRect;
}

/** An element for the claim listeners, which only need to be an event target. */
function pane(): HTMLElement {
  return new EventTarget() as unknown as HTMLElement;
}

describe("topmostOffset", () => {
  const blocks = [
    { start: 20, top: -60, height: 40 }, // scrolled fully past
    { start: 44, top: -20, height: 40 }, // straddling the top edge
    { start: 120, top: 20, height: 40 },
  ];

  it("takes the first block still on screen", () => {
    // Viewport top 0: the straddling block is the one being read, not the one
    // above it that has gone entirely.
    expect(topmostOffset(container(blocks, 0))).toBe(44);
  });

  it("ignores a block clipped by a pixel", () => {
    const nearly = [
      { start: 20, top: -38, height: 40 }, // bottom at 2, inside the tolerance
      { start: 44, top: 2, height: 40 },
    ];
    expect(topmostOffset(container(nearly, 0))).toBe(44);
  });

  it("falls back to the last block at the bottom of the document", () => {
    // Everything has scrolled above the top edge, which is what the final
    // screen of a document looks like. The honest answer is the end.
    const scrolledPast = blocks.map((block) => ({ ...block, top: -200 }));
    expect(topmostOffset(container(scrolledPast, 0))).toBe(120);
  });

  it("is null with nothing to measure", () => {
    expect(topmostOffset(container([]))).toBeNull();
  });
});

describe("elementFor", () => {
  const blocks = [
    { start: 20, top: 0, height: 10 },
    { start: 44, top: 10, height: 10 },
    { start: 505, top: 20, height: 10 },
  ];

  it("takes the block an offset falls inside", () => {
    expect(elementFor(container(blocks), 300)?.dataset.start).toBe("44");
  });

  it("takes the block starting exactly at the offset", () => {
    expect(elementFor(container(blocks), 44)?.dataset.start).toBe("44");
  });

  it("takes the first block for an offset before all of them", () => {
    // The top of every document: `\id` and the rest of the header are before
    // the first block that exists. Answering "nothing" left the preview
    // wherever it had been, so scrolling the source to the top did nothing.
    expect(elementFor(container(blocks), 0)?.dataset.start).toBe("20");
  });

  it("takes the last block past the end", () => {
    expect(elementFor(container(blocks), 99999)?.dataset.start).toBe("505");
  });

  it("is null with nothing to choose from", () => {
    expect(elementFor(container([]), 10)).toBeNull();
  });
});

describe("ScrollSync", () => {
  let sync: ScrollSync;
  let element: HTMLElement;

  beforeEach(() => {
    sync = new ScrollSync();
    element = pane();
  });

  it("accepts nothing until a pane is claimed", () => {
    // A scroll nobody asked for — a reveal, a resize, a font arriving — moves
    // neither pane, which is the safe answer.
    expect(sync.accepts("editor")).toBe(false);
    expect(sync.accepts("preview")).toBe(false);
  });

  it("accepts the pane the user is scrolling", () => {
    sync.watch("editor", element);
    element.dispatchEvent(new Event("wheel"));

    expect(sync.accepts("editor")).toBe(true);
  });

  it("refuses the pane that was moved for it", () => {
    // The whole guard. The editor is being scrolled, so the preview's own
    // scroll events are echoes of that and must move nothing back.
    sync.watch("editor", element);
    element.dispatchEvent(new Event("wheel"));

    expect(sync.accepts("preview")).toBe(false);
    // And repeatedly: a virtualized editor corrects itself over several
    // frames, and every one of those is still an echo.
    expect(sync.accepts("preview")).toBe(false);
    expect(sync.accepts("preview")).toBe(false);
  });

  it("hands over when the user moves to the other pane", () => {
    const other = pane();
    sync.watch("editor", element);
    sync.watch("preview", other);

    element.dispatchEvent(new Event("wheel"));
    expect(sync.accepts("editor")).toBe(true);

    other.dispatchEvent(new Event("pointerdown"));
    expect(sync.accepts("preview")).toBe(true);
    expect(sync.accepts("editor")).toBe(false);
  });

  it("claims on every kind of input that scrolls", () => {
    for (const type of ["wheel", "pointerdown", "keydown", "touchstart"]) {
      const fresh = new ScrollSync();
      fresh.watch("preview", element);
      element.dispatchEvent(new Event(type));
      expect(fresh.accepts("preview"), type).toBe(true);
    }
  });

  it("stops claiming once unwatched", () => {
    const stop = sync.watch("editor", element);
    stop();
    element.dispatchEvent(new Event("wheel"));

    expect(sync.accepts("editor")).toBe(false);
  });
});
