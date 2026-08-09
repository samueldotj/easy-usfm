/**
 * The zoom arithmetic.
 *
 * Two properties worth writing down. Stepping out and back in has to return to
 * exactly where it started -- repeated multiplication and division drifts, and
 * a user who zooms in ten times and out ten times should be at 100%, not at
 * 99.9% with no way to tell. And the bounds have to hold, because below the
 * floor the interface stops being readable and above the ceiling a line holds
 * one word.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { zoom } from "./zoom.svelte";

beforeEach(() => {
  vi.stubGlobal("localStorage", {
    getItem: () => null,
    setItem: () => {},
  });
  vi.stubGlobal("document", { documentElement: { style: { setProperty: () => {} } } });
  zoom.reset();
});

describe("stepping", () => {
  it("returns to exactly where it started", () => {
    for (let step = 0; step < 10; step += 1) zoom.in();
    for (let step = 0; step < 10; step += 1) zoom.out();

    expect(zoom.level).toBe(1);
    expect(zoom.percent).toBe(100);
  });

  it("grows and shrinks", () => {
    zoom.in();
    expect(zoom.level).toBeGreaterThan(1);

    zoom.reset();
    zoom.out();
    expect(zoom.level).toBeLessThan(1);
  });

  it("stops at the bounds rather than running away", () => {
    for (let step = 0; step < 100; step += 1) zoom.out();
    expect(zoom.level).toBeGreaterThanOrEqual(0.6);

    for (let step = 0; step < 200; step += 1) zoom.in();
    expect(zoom.level).toBeLessThanOrEqual(3);
  });
});

describe("the wheel", () => {
  const wheel = (init: Partial<WheelEvent>) =>
    ({ ctrlKey: false, metaKey: false, deltaY: 0, preventDefault: () => {}, ...init }) as WheelEvent;

  it("ignores a plain scroll", () => {
    // Otherwise reading a document would resize it.
    expect(zoom.wheel(wheel({ deltaY: -120 }))).toBe(false);
    expect(zoom.level).toBe(1);
  });

  it("zooms with the modifier held", () => {
    expect(zoom.wheel(wheel({ ctrlKey: true, deltaY: -120 }))).toBe(true);
    expect(zoom.level).toBeGreaterThan(1);

    zoom.wheel(wheel({ ctrlKey: true, deltaY: 120 }));
    expect(zoom.level).toBe(1);
  });

  it("prevents the browser's own zoom", () => {
    // Without this the page zooms as well and the two fight.
    let prevented = false;
    zoom.wheel(wheel({ ctrlKey: true, deltaY: -120, preventDefault: () => (prevented = true) }));
    expect(prevented).toBe(true);
  });

  it("uses the direction, not the magnitude", () => {
    // Wheel deltas differ wildly between a notched mouse, a trackpad and a
    // hi-res wheel; using the magnitude makes one gesture jump differently on
    // different hardware.
    zoom.wheel(wheel({ ctrlKey: true, deltaY: -1 }));
    const small = zoom.level;

    zoom.reset();
    zoom.wheel(wheel({ ctrlKey: true, deltaY: -4000 }));
    expect(zoom.level).toBe(small);
  });
});

describe("reporting", () => {
  it("says nothing is changed at actual size", () => {
    expect(zoom.changed).toBe(false);
  });

  it("says so once it is", () => {
    zoom.in();
    expect(zoom.changed).toBe(true);
  });
});
