/**
 * Keeping the two panes together — P3.6.
 *
 * Both panes show the same document, so scrolling one and not the other makes
 * the split useless the moment a book is longer than a screen: the translator
 * scrolls the source, looks right, and the preview is showing chapter one.
 *
 * # The loop is the whole problem
 *
 * Syncing A to B makes B scroll, which asks to sync B to A, which makes A
 * scroll. The two panes then chase each other — visibly, as a shudder, and on
 * a long document they drift apart while doing it because their line heights
 * differ.
 *
 * The obvious guard is a flag, and it does not work: setting `scrollTop` does
 * not raise `scroll` synchronously, so a flag cleared at the end of the
 * function is already false when the echo arrives. The next guess is a
 * deadline — ignore the moved pane's scroll events for a moment — and that one
 * is worse, because it looks correct until it is measured. The editor is
 * virtualized: it scrolls, renders the lines it landed on, discovers their real
 * heights, and corrects itself, emitting scroll events across several frames.
 * Any deadline short enough to keep the sync responsive is short enough for one
 * of those corrections to land outside it and be read as a new intent. Driving
 * the preview to offset 700 and watching it snap back to 178 is what that looks
 * like.
 *
 * So the guard here is not about timing, it is about *who asked*. A pane syncs
 * the other only while it is the one the user is actually working in, which the
 * browser already reports: wheel, pointer, and key events land on the pane
 * under the hand. A pane that was moved programmatically never claims the
 * wheel, so it can emit as many corrective scroll events as it likes and none
 * of them moves anything. The loop cannot form, rather than being outrun.
 */

/**
 * How long a pane keeps the wheel after the user's last input to it.
 *
 * Long enough to cover the tail of a fling and the gap between two wheel
 * notches, short enough that moving to the other pane and scrolling it takes
 * effect on the first notch. Any input to a pane restarts it, so a continuous
 * gesture never expires mid-way.
 */
const HOLD_MS = 400;

export type Pane = "editor" | "preview";

/** The events that mean "the user is scrolling *this* one". */
const INTENTS = ["wheel", "pointerdown", "keydown", "touchstart"] as const;

export class ScrollSync {
  /** The pane the user is working in, and how long it keeps that claim. */
  #driver: Pane | null = null;
  #until = 0;

  /**
   * Whether a scroll event on `pane` should move the other one.
   *
   * False for every pane that is not the driver — which is the whole guard.
   */
  accepts(pane: Pane): boolean {
    if (this.#driver !== pane) return false;

    const now = performance.now();
    if (now > this.#until) {
      this.#driver = null;
      return false;
    }

    // A scroll from the driver extends its claim: a long drag on a scrollbar
    // is one gesture, and it emits scroll events without emitting a second
    // `pointerdown`.
    this.#until = now + HOLD_MS;
    return true;
  }

  /**
   * Watches `element` for the user starting to scroll it.
   *
   * Attached here rather than in the components so that the rule about who
   * drives lives in one place with the reason for it. Passive and capturing:
   * nothing is prevented, and a claim has to be recorded even when something
   * inside the pane stops the event.
   */
  watch(pane: Pane, element: HTMLElement): () => void {
    const claim = () => {
      this.#driver = pane;
      this.#until = performance.now() + HOLD_MS;
    };

    for (const type of INTENTS) {
      element.addEventListener(type, claim, { passive: true, capture: true });
    }

    return () => {
      for (const type of INTENTS) {
        element.removeEventListener(type, claim, { capture: true });
      }
    };
  }
}

/**
 * The offset of the first block at or below the top of the preview.
 *
 * "At or below" rather than "nearest": a chapter heading half scrolled off is
 * not what the reader is looking at, and rounding towards what has already
 * gone past scrolls the editor backwards.
 *
 * Falling back to the last block when nothing qualifies, which happens at the
 * very bottom where the final paragraph has scrolled above the top edge. The
 * honest answer there is the end of the document, and returning nothing would
 * leave the editor wherever it happened to be.
 */
export function topmostOffset(container: HTMLElement): number | null {
  const top = container.getBoundingClientRect().top;
  const blocks = container.querySelectorAll<HTMLElement>("[data-start]");

  for (const element of blocks) {
    const box = element.getBoundingClientRect();
    // A small tolerance, because a block whose first line is clipped by a
    // pixel is still the one being read.
    if (box.bottom > top + 4) return offsetOf(element);
  }

  const last = blocks[blocks.length - 1];
  return last ? offsetOf(last) : null;
}

/**
 * The element to scroll to for a source offset.
 *
 * The last block that begins at or before it, since an offset in the middle of
 * a paragraph belongs to that paragraph rather than to the next one.
 *
 * The first block when the offset precedes all of them, which is not an edge
 * case but the top of every document: `\id` and the rest of the header carry no
 * block of their own, so scrolling the editor to the very top — the single most
 * common navigation there is — asks about an offset no element claims. Answering
 * "nothing" left the preview wherever it had been.
 *
 * A linear walk. The alternative is an index rebuilt on every render, to
 * answer a question asked once per scroll event over a list of blocks whose
 * length is a chapter — the index would cost more than it saves.
 */
export function elementFor(container: HTMLElement, offset: number): HTMLElement | null {
  let best: HTMLElement | null = null;

  for (const element of container.querySelectorAll<HTMLElement>("[data-start]")) {
    const start = Number(element.dataset.start);
    if (!Number.isFinite(start)) continue;
    if (start > offset) return best ?? element;
    best = element;
  }
  return best;
}

/** Puts `element` at the top of its scrolling container. */
export function scrollTo(container: HTMLElement, element: HTMLElement): void {
  const delta = element.getBoundingClientRect().top - container.getBoundingClientRect().top;
  container.scrollTop += delta;
}

function offsetOf(element: HTMLElement): number | null {
  const start = Number(element.dataset.start);
  return Number.isFinite(start) ? start : null;
}
