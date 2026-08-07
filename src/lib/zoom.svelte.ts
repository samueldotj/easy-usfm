/**
 * Zoom — Ctrl and the wheel, or Ctrl with plus and minus.
 *
 * Scripture is read for hours at a time, often by someone whose eyes are not
 * what they were, and often in a script whose glyphs carry detail Latin does
 * not. Being able to make the text bigger is not a convenience here.
 *
 * # One variable, not two font settings
 *
 * The editor and the preview both inherit their size, so a single custom
 * property on the root scales both together. That matters more than it sounds:
 * the two panes are the same document, and a translator comparing them needs
 * them at the same size — two independent zooms would let them drift and give
 * nobody a way to put them back.
 *
 * The property is set through CSSOM rather than a stylesheet because the value
 * is a number chosen at runtime. It is *not* a style element, which the real
 * policy blocks (SECURITY §4).
 *
 * # Steps, not a slider
 *
 * Multiplying by a fixed ratio each step keeps the change proportional at every
 * size — going from 0.8 to 0.9 is a bigger jump than 2.0 to 2.1, so a linear
 * step feels coarse when small and useless when large. The ratio is the same
 * one browsers use for their own zoom, so it lands on familiar sizes.
 */

import { read, write } from "./settings";

/** One step. The ratio browsers use, so the sizes feel familiar. */
const STEP = 1.1;

/** Below this the interface stops being readable; above it, one word a line. */
const MIN = 0.6;
const MAX = 3;

const KEY = "zoom";

const isLevel = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value) && value >= MIN && value <= MAX;

/** The custom property the panes multiply their size by. */
const PROPERTY = "--zoom";

class Zoom {
  level = $state(read(KEY, 1, isLevel));

  /** Whether the level is anything other than the default, for the interface. */
  get changed(): boolean {
    return Math.abs(this.level - 1) > 0.001;
  }

  /** As a percentage, for showing in the status bar. */
  get percent(): number {
    return Math.round(this.level * 100);
  }

  /** Puts the current level where the stylesheet can see it. */
  apply(): void {
    document.documentElement.style.setProperty(PROPERTY, String(this.level));
  }

  in(): void {
    this.set(this.level * STEP);
  }

  out(): void {
    this.set(this.level / STEP);
  }

  reset(): void {
    this.set(1);
  }

  /**
   * Sets the level, clamped and stored.
   *
   * Rounded to three places so the stored value does not accumulate the drift
   * of repeated multiplication — twenty steps in and twenty back should return
   * to exactly one, not to 0.9999999.
   */
  set(level: number): void {
    const clamped = Math.min(Math.max(level, MIN), MAX);
    this.level = Math.round(clamped * 1000) / 1000;

    this.apply();
    write(KEY, this.level);
  }

  /**
   * A wheel event that means zoom, or one that means scroll.
   *
   * `ctrlKey` is how every editor spells this, and it is also what the browser
   * reads as page zoom — so the event has to be prevented, which needs a
   * non-passive listener. Returns whether it was handled, so the caller knows
   * whether to let the pane scroll.
   *
   * macOS pinch gestures arrive here as a wheel event with `ctrlKey` set and no
   * key held, which is exactly right: pinching should zoom.
   */
  wheel(event: WheelEvent): boolean {
    if (!event.ctrlKey && !event.metaKey) return false;

    event.preventDefault();
    // Direction only. Wheel deltas are wildly inconsistent between a notched
    // mouse, a trackpad and a hi-res wheel, so using the magnitude makes the
    // same gesture jump differently on different hardware.
    if (event.deltaY < 0) this.in();
    else if (event.deltaY > 0) this.out();
    return true;
  }
}

export const zoom = new Zoom();
