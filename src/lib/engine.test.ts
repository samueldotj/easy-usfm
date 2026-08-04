import { describe, expect, it } from "vitest";

import { checksum } from "./checksum";
import { CHECKSUM_INTERVAL } from "./delta";
import { Engine, type WorkerLike } from "./engine.svelte";
import type { ParseResult, Request, Response } from "../worker/protocol";

/**
 * A worker that behaves like the real one, and can be made to misbehave.
 *
 * The real engine's mirror is a Rust `Session`; this is the same protocol over
 * a string. What it adds is the ability to *drop* an edit, which is how drift
 * is induced — the thing a correct system never does on its own, and the thing
 * the checksum exists to catch when something else causes it.
 */
class FakeWorker implements WorkerLike {
  onmessage: ((event: { data: Response }) => void) | null = null;

  mirror = "";
  received: Request[] = [];
  /** Replies held back, to be delivered out of order. */
  held: Response[] = [];

  #dropNext = false;
  #hold = false;
  terminated = false;

  /** The next edit arrives and is quietly ignored. */
  dropNextEdit(): void {
    this.#dropNext = true;
  }

  /** Stop delivering replies; they queue in `held`. */
  holdReplies(): void {
    this.#hold = true;
  }

  /** Deliver held replies in the given order, by index. */
  deliver(order: number[]): void {
    this.#hold = false;
    for (const index of order) this.#emit(this.held[index]!);
    this.held = [];
  }

  postMessage(message: unknown): void {
    const request = message as Request;
    this.received.push(request);

    switch (request.kind) {
      case "version":
        this.#emit({ kind: "version", rev: request.rev, version: "test" });
        return;

      case "open":
      case "resync":
        this.mirror = request.text;
        this.#emit({ kind: "parsed", rev: request.rev, result: this.#result(request.rev) });
        return;

      case "edit": {
        for (const edit of request.edits) {
          if (this.#dropNext) {
            // The induced fault: acknowledge the batch, apply nothing.
            this.#dropNext = false;
            continue;
          }
          this.mirror =
            this.mirror.slice(0, edit.from) + edit.insert + this.mirror.slice(edit.to);
        }

        if (request.checksum !== undefined && checksum(this.mirror) !== request.checksum) {
          this.#emit({ kind: "desync", rev: request.rev, reason: "checksum mismatch" });
          return;
        }
        this.#emit({ kind: "parsed", rev: request.rev, result: this.#result(request.rev) });
        return;
      }
    }
  }

  terminate(): void {
    this.terminated = true;
  }

  #emit(response: Response): void {
    if (this.#hold) {
      this.held.push(response);
      return;
    }
    this.onmessage?.({ data: response });
  }

  #result(rev: number): ParseResult {
    return {
      rev,
      chunks: [],
      diagnostics: [
        {
          code: "TEST-001",
          severity: "information",
          start: 0,
          end: this.mirror.length,
          message: this.mirror,
        },
      ],
      len: this.mirror.length,
    };
  }
}

function connected(): { engine: Engine; worker: FakeWorker } {
  const worker = new FakeWorker();
  const engine = new Engine();
  engine.start(() => worker);
  // The real worker announces itself once the module instantiates.
  worker.onmessage?.({ data: { kind: "ready" } });
  return { engine, worker };
}

// ------------------------------------------------------------------ P2.4 ---

describe("desync detection and repair", () => {
  it("catches induced drift within CHECKSUM_INTERVAL batches", () => {
    const { engine, worker } = connected();
    engine.open("\\id GEN\n");

    let editor = worker.mirror;
    worker.dropNextEdit();

    // Detection is observed by the resync it provokes, not by the `desynced`
    // flag: repair is immediate, so the flag is set and cleared inside the
    // same call. That is the design working, and it makes the flag useless as
    // a probe.
    let caught = -1;
    for (let index = 0; index < CHECKSUM_INTERVAL; index += 1) {
      const at = editor.length;
      editor = `${editor}x`;
      engine.edit([{ from: at, to: at, insert: "x" }], editor);

      if (worker.received.some((request) => request.kind === "resync")) {
        caught = index;
        break;
      }
    }

    expect(caught).toBeGreaterThanOrEqual(0);
    expect(caught).toBeLessThan(CHECKSUM_INTERVAL);
  });

  it("repairs without losing what the editor holds", () => {
    const { engine, worker } = connected();
    engine.open("\\id GEN\n");

    let editor = worker.mirror;
    worker.dropNextEdit();

    // Type past the checksum boundary so the drift is found and repaired.
    for (let index = 0; index < CHECKSUM_INTERVAL + 1; index += 1) {
      const at = editor.length;
      editor = `${editor}${index % 10}`;
      engine.edit([{ from: at, to: at, insert: String(index % 10) }], editor);
    }

    // The editor is authoritative (ADR-003); repair means the mirror is made
    // to match it, never the other way round.
    expect(worker.mirror).toBe(editor);
    expect(engine.desynced).toBeNull();
    expect(worker.received.some((request) => request.kind === "resync")).toBe(true);
  });

  it("does not send more edits onto a mirror it does not trust", () => {
    const { engine, worker } = connected();
    engine.open("abc");

    // Replies are held so the repair cannot complete, which is the real
    // situation: the worker is asynchronous, so there is always a window
    // between noticing the drift and having fixed it. Edits arriving in that
    // window must not be sent — they would be applied to a document the
    // engine does not have.
    worker.holdReplies();
    worker.onmessage?.({ data: { kind: "desync", rev: 99, reason: "induced" } });

    const before = worker.received.length;
    engine.edit([{ from: 0, to: 0, insert: "z" }], "zabc");
    engine.edit([{ from: 1, to: 1, insert: "y" }], "zyabc");

    const sent = worker.received.slice(before);
    expect(sent.length).toBeGreaterThan(0);
    expect(sent.every((request) => request.kind === "resync")).toBe(true);
  });
});

// ------------------------------------------------------------------ P2.5 ---

describe("revision discipline", () => {
  it("never applies a result older than one already applied", () => {
    const { engine, worker } = connected();
    engine.open("first");

    worker.holdReplies();
    engine.edit([{ from: 5, to: 5, insert: "A" }], "firstA");
    engine.edit([{ from: 6, to: 6, insert: "B" }], "firstAB");

    // Delivered newest first, which is what a loaded worker does under
    // fast typing: the later reply wins the race.
    worker.deliver([1, 0]);

    // The stale reply must not have overwritten the newer one. The fake puts
    // its mirror in the diagnostic message, so the message says which won.
    expect(engine.diagnostics[0]?.message).toBe("firstAB");
  });

  it("survives a scripted fast-typing load with results arriving out of order", () => {
    const { engine, worker } = connected();
    engine.open("");

    let editor = "";
    worker.holdReplies();

    for (let index = 0; index < 200; index += 1) {
      const at = editor.length;
      editor = `${editor}${String.fromCharCode(97 + (index % 26))}`;
      engine.edit([{ from: at, to: at, insert: editor[at]! }], editor);
    }

    // Shuffled deterministically, so a failure reproduces.
    const order = worker.held.map((_, index) => index);
    for (let index = order.length - 1; index > 0; index -= 1) {
      const swap = (index * 7 + 3) % (index + 1);
      [order[index], order[swap]] = [order[swap]!, order[index]!];
    }
    worker.deliver(order);

    // Whatever order they arrived in, what is displayed is the newest.
    expect(engine.diagnostics[0]?.message).toBe(editor);
  });

  it("stops cleanly", () => {
    const { engine, worker } = connected();
    engine.stop();
    expect(worker.terminated).toBe(true);
    expect(engine.ready).toBe(false);
  });
});
