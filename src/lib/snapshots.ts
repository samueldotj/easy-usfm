/**
 * When to take a recovery snapshot — FILE-FIDELITY §4, P4.1.
 *
 * "On 4 s idle after a change, and unconditionally every 45 s during continuous
 * typing."
 *
 * Two rules rather than one, because either alone fails a real case. Idle-only
 * never fires for someone drafting steadily for ten minutes — exactly the
 * session with the most to lose. A fixed interval alone writes a snapshot every
 * 45 seconds of a document nobody is editing, which is disk churn for nothing
 * and, on a laptop, a reason for the drive to stay awake.
 *
 * So: a change starts an idle timer, and the *first* change after a snapshot
 * also starts a ceiling. Whichever comes first wins, and taking a snapshot
 * clears both.
 *
 * # Kept separate from the shell
 *
 * This is a scheduler and nothing else: it does not know what a document is,
 * where snapshots go, or whether the host can store one. That is what makes it
 * testable against a clock rather than against a filesystem, and it is why the
 * same object serves the desktop and, later, the web (P4.6) — the cadence is
 * the same on both and only the destination differs.
 */

/** On 4 s idle after a change. */
export const IDLE_MS = 4_000;

/** And unconditionally every 45 s during continuous typing. */
export const CEILING_MS = 45_000;

/** The timer functions, so tests can drive a clock instead of waiting. */
export interface Timers {
  set(callback: () => void, ms: number): number;
  clear(handle: number): void;
}

const realTimers: Timers = {
  set: (callback, ms) => setTimeout(callback, ms) as unknown as number,
  clear: (handle) => clearTimeout(handle),
};

export class SnapshotSchedule {
  #idle: number | null = null;
  #ceiling: number | null = null;

  readonly #take: () => void;
  readonly #timers: Timers;

  constructor(take: () => void, timers: Timers = realTimers) {
    this.#take = take;
    this.#timers = timers;
  }

  /**
   * The document changed.
   *
   * The idle timer restarts on every change — that is what makes it idle. The
   * ceiling does not: restarting it on each keystroke would mean it never
   * expires under continuous typing, which is the one case it exists for.
   */
  changed(): void {
    if (this.#idle !== null) this.#timers.clear(this.#idle);
    this.#idle = this.#timers.set(() => this.#fire(), IDLE_MS);

    this.#ceiling ??= this.#timers.set(() => this.#fire(), CEILING_MS);
  }

  /**
   * Nothing is outstanding: a clean save, or a document being closed.
   *
   * Cancels rather than flushes. A snapshot taken right after a save would
   * record work that is already on disk, and FILE-FIDELITY §4 has snapshots
   * *cleared* at that moment — writing one instead would be the opposite.
   */
  settled(): void {
    this.#cancel();
  }

  /**
   * Takes one now, if anything is outstanding.
   *
   * For teardown — `visibilitychange` on the web, closing a window on the
   * desktop. Does nothing when there is nothing pending, so a close with no
   * unsaved work does not leave a snapshot behind for the next launch to
   * offer back.
   */
  flush(): void {
    if (this.#idle === null && this.#ceiling === null) return;
    this.#fire();
  }

  #fire(): void {
    this.#cancel();
    this.#take();
  }

  #cancel(): void {
    if (this.#idle !== null) this.#timers.clear(this.#idle);
    if (this.#ceiling !== null) this.#timers.clear(this.#ceiling);
    this.#idle = null;
    this.#ceiling = null;
  }
}
