/**
 * The snapshot cadence — FILE-FIDELITY §4, P4.1.
 *
 * Driven against a fake clock rather than real timers, because the property
 * being tested is *when*, and a test that waits 45 seconds to find out is a
 * test nobody runs.
 *
 * The case worth writing down is the ceiling. Someone drafting steadily never
 * goes idle, so an idle-only schedule quietly never snapshots the session with
 * the most unsaved work in it — a bug that passes every test written by typing
 * a few characters and waiting.
 */

import { describe, expect, it } from "vitest";

import { CEILING_MS, IDLE_MS, SnapshotSchedule, type Timers } from "./snapshots";

/** A clock that only moves when told to. */
function clock() {
  let now = 0;
  let next = 1;
  const pending = new Map<number, { at: number; callback: () => void }>();

  const timers: Timers = {
    set(callback, ms) {
      const handle = next++;
      pending.set(handle, { at: now + ms, callback });
      return handle;
    },
    clear(handle) {
      pending.delete(handle);
    },
  };

  return {
    timers,
    /** Moves time forward, running whatever comes due, in order. */
    advance(ms: number) {
      const until = now + ms;
      for (;;) {
        const due = [...pending.entries()]
          .filter(([, timer]) => timer.at <= until)
          .sort((left, right) => left[1].at - right[1].at)[0];
        if (!due) break;

        const [handle, timer] = due;
        pending.delete(handle);
        now = timer.at;
        timer.callback();
      }
      now = until;
    },
  };
}

function schedule() {
  const taken: number[] = [];
  const time = clock();
  const under = new SnapshotSchedule(() => taken.push(taken.length + 1), time.timers);
  return { taken, time, under };
}

describe("idle", () => {
  it("snapshots four seconds after a change", () => {
    const { taken, time, under } = schedule();

    under.changed();
    time.advance(IDLE_MS - 1);
    expect(taken).toHaveLength(0);

    time.advance(1);
    expect(taken).toHaveLength(1);
  });

  it("restarts on every change, which is what makes it idle", () => {
    const { taken, time, under } = schedule();

    // Typing every three seconds: never idle, so the idle rule never fires.
    for (let round = 0; round < 5; round += 1) {
      under.changed();
      time.advance(3_000);
    }
    expect(taken).toHaveLength(0);

    time.advance(IDLE_MS);
    expect(taken).toHaveLength(1);
  });

  it("does nothing on its own", () => {
    const { taken, time } = schedule();

    time.advance(CEILING_MS * 3);
    expect(taken).toHaveLength(0);
  });
});

describe("the ceiling", () => {
  it("fires under continuous typing, when idle never would", () => {
    const { taken, time, under } = schedule();

    // A keystroke every second for a minute. The idle timer is reset each
    // time and never expires; without the ceiling this session -- the one with
    // the most to lose -- would never be snapshotted at all.
    for (let second = 0; second < 60; second += 1) {
      under.changed();
      time.advance(1_000);
    }

    expect(taken.length).toBeGreaterThanOrEqual(1);
  });

  it("is not restarted by a change", () => {
    const { taken, time, under } = schedule();

    under.changed();
    time.advance(CEILING_MS - 1_000);
    // A change this late must not push the ceiling out; that is the whole
    // difference between it and the idle timer.
    under.changed();
    time.advance(1_000);

    expect(taken).toHaveLength(1);
  });

  it("starts again from the next change after a snapshot", () => {
    const { taken, time, under } = schedule();

    for (let second = 0; second < 120; second += 1) {
      under.changed();
      time.advance(1_000);
    }

    // Two ceilings in two minutes of unbroken typing, not one.
    expect(taken.length).toBeGreaterThanOrEqual(2);
  });
});

describe("settling", () => {
  it("cancels rather than flushes", () => {
    // A clean save clears snapshots (FILE-FIDELITY §4). Writing one at that
    // moment would be the exact opposite of what the section asks for.
    const { taken, time, under } = schedule();

    under.changed();
    under.settled();
    time.advance(CEILING_MS * 2);

    expect(taken).toHaveLength(0);
  });

  it("leaves the schedule ready for the next change", () => {
    const { taken, time, under } = schedule();

    under.changed();
    under.settled();
    under.changed();
    time.advance(IDLE_MS);

    expect(taken).toHaveLength(1);
  });
});

describe("flush", () => {
  it("takes one when something is outstanding", () => {
    const { taken, under } = schedule();

    under.changed();
    under.flush();

    expect(taken).toHaveLength(1);
  });

  it("does nothing when nothing is", () => {
    // Closing a window with no unsaved work must not leave a snapshot for the
    // next launch to offer back.
    const { taken, under } = schedule();

    under.flush();

    expect(taken).toHaveLength(0);
  });

  it("does not leave a timer behind", () => {
    const { taken, time, under } = schedule();

    under.changed();
    under.flush();
    time.advance(CEILING_MS * 2);

    expect(taken).toHaveLength(1);
  });
});
