/**
 * The delta protocol — ARCHITECTURE §9.
 *
 * The text is mirrored, not shipped. CodeMirror is authoritative for editing
 * and the worker holds a synchronized copy; sending a 2 MB string per debounce
 * would mean a transcode and an allocation on every edit.
 *
 * Three things happen here, and the second is a correctness requirement rather
 * than an optimisation.
 *
 * **Batching.** Edits from one transaction go over as one message.
 *
 * **Composition suppression.** Input methods — transliteration, InScript
 * layouts, platform IMEs — produce intermediate states before they commit.
 * CodeMirror emits transactions for those. Sending them means the mirror
 * receives text the user never committed, the preview flickers through partial
 * syllables, and worst, some platforms' composition teardown produces no clean
 * inverse transaction, so the mirror desyncs and the checksum forces a full
 * resync mid-typing, on every word (UNICODE §5). Edits are buffered while
 * composing and flushed as one coalesced batch on commit.
 *
 * **Desync detection.** A checksum rides along every 50 batches and at each
 * idle boundary. Silent drift corrupts every offset in the interface, so the
 * cost of checking is worth paying and the cost of *not* checking is
 * unbounded.
 */

import { checksum } from "./checksum";
import type { Edit } from "../worker/protocol";

/** How many batches may pass before a checksum is included. */
export const CHECKSUM_INTERVAL = 50;

export interface Batch {
  edits: Edit[];
  /** Present when this batch carries a verification checksum. */
  checksum?: number;
}

/**
 * Collects edits into batches, suppressing them while an input method is
 * composing.
 *
 * Deliberately free of CodeMirror and of the worker: it takes edits and text
 * and returns batches, which is what makes the composition behaviour testable
 * without a browser or an input method.
 */
export class DeltaBuffer {
  #pending: Edit[] = [];
  #composing = false;
  #sinceChecksum = 0;

  get composing(): boolean {
    return this.#composing;
  }

  /** Edits held back because an input method is mid-word. */
  get pendingCount(): number {
    return this.#pending.length;
  }

  /**
   * Records edits from one transaction.
   *
   * Returns the batch to send, or `null` while composing — the caller sends
   * nothing at all rather than sending something provisional.
   */
  push(edits: Edit[], text: string): Batch | null {
    if (edits.length === 0) return null;

    this.#pending.push(...edits);
    if (this.#composing) return null;

    return this.#flush(text);
  }

  /** An input method has started. Everything is held until it commits. */
  startComposition(): void {
    this.#composing = true;
  }

  /**
   * An input method has committed.
   *
   * Returns one batch for everything it produced. One, not several: the point
   * is that the mirror sees the committed word and never the syllables it was
   * assembled from.
   */
  endComposition(text: string): Batch | null {
    this.#composing = false;
    if (this.#pending.length === 0) return null;
    return this.#flush(text);
  }

  /**
   * Typing has stopped. Sends anything held, with a checksum.
   *
   * The idle boundary is the cheap place to verify: nothing is competing for
   * the main thread, and a drift caught here is caught before the next
   * keystroke builds on it.
   */
  idle(text: string): Batch | null {
    if (this.#pending.length === 0) {
      // Nothing to send, but the mirror is still worth checking — drift can
      // come from a dropped message rather than from an edit.
      this.#sinceChecksum = 0;
      return { edits: [], checksum: checksum(text) };
    }
    this.#sinceChecksum = CHECKSUM_INTERVAL;
    return this.#flush(text);
  }

  #flush(text: string): Batch {
    const edits = this.#pending;
    this.#pending = [];

    this.#sinceChecksum += 1;
    if (this.#sinceChecksum >= CHECKSUM_INTERVAL) {
      this.#sinceChecksum = 0;
      return { edits, checksum: checksum(text) };
    }
    return { edits };
  }
}

/**
 * A model of the worker's mirror, for testing.
 *
 * Applies the same batches the worker would and reports what it holds. The
 * engine's own mirror is the Rust `Session`; this exists so the *protocol* can
 * be tested without one — if a sequence of batches does not reconstruct the
 * document here, it will not reconstruct it there either.
 */
export function applyToMirror(mirror: string, batch: Batch): string {
  let text = mirror;
  for (const edit of batch.edits) {
    text = text.slice(0, edit.from) + edit.insert + text.slice(edit.to);
  }
  return text;
}
