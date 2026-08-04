/**
 * The main thread's half of the engine conversation.
 *
 * Holds `latestAppliedRev` and discards anything older (ARCHITECTURE §8.3).
 * The worker answers out of order under load, and a late reply carrying stale
 * offsets would light up diagnostics on text that has since been retyped.
 */

import type { Diagnostic, Edit, ParseResult, Request, Response } from "../worker/protocol";

export type { Diagnostic } from "../worker/protocol";

class Engine {
  /** The engine's version, once the worker has answered. */
  version = $state<string | null>(null);
  ready = $state(false);
  diagnostics = $state<Diagnostic[]>([]);
  chunks = $state<ParseResult["chunks"]>([]);

  /**
   * Set when the worker's mirror stopped matching the editor. The document is
   * still safe — the buffer is authoritative (ADR-003) — but nothing the
   * engine says about it can be trusted until a resync lands.
   */
  desynced = $state<string | null>(null);

  #worker: Worker | null = null;
  #rev = 0;
  #applied = 0;
  /** The last text sent, so a desync can be repaired without asking anyone. */
  #text = "";

  start(): void {
    if (this.#worker) return;

    this.#worker = new Worker(new URL("../worker/engine.worker.ts", import.meta.url), {
      type: "module",
      name: "easy-usfm-engine",
    });

    this.#worker.onmessage = (event: MessageEvent<Response>) => this.#receive(event.data);
    // Nothing is asked of the engine until it says it is ready. The WASM
    // module is fetched and instantiated asynchronously, so a request sent at
    // construction reaches a worker whose exports do not exist yet — it throws,
    // is caught as a desync, and the answer is simply lost.
  }

  stop(): void {
    this.#worker?.terminate();
    this.#worker = null;
    this.ready = false;
  }

  open(text: string): void {
    this.#text = text;
    // Held until the module is ready; `ready` sends it. Queuing here rather
    // than making every caller wait keeps the document lifecycle free of the
    // engine's startup.
    if (!this.ready) return;
    this.#send({ kind: "open", rev: this.#next(), text });
  }

  edit(edits: Edit[], text: string): void {
    this.#text = text;
    if (this.desynced) {
      // Already broken; sending more edits onto a mirror we do not trust
      // cannot help. Repair instead.
      this.#resync();
      return;
    }
    this.#send({ kind: "edit", rev: this.#next(), edits });
  }

  #resync(): void {
    this.#send({ kind: "resync", rev: this.#next(), text: this.#text });
  }

  #next(): number {
    this.#rev += 1;
    return this.#rev;
  }

  #send(request: Request): void {
    this.#worker?.postMessage(request);
  }

  #receive(response: Response): void {
    switch (response.kind) {
      case "ready":
        this.ready = true;
        this.#send({ kind: "version", rev: this.#next() });
        // A document opened before the module finished loading was never
        // parsed, so it is sent now rather than waiting for the first
        // keystroke to reveal that the panel is empty.
        if (this.#text) this.#resync();
        return;

      case "version":
        this.version = response.version;
        return;

      case "desync":
        this.desynced = response.reason;
        // Repair immediately rather than waiting for the next keystroke: a
        // desynced engine shows stale diagnostics until it is fixed.
        this.#resync();
        return;

      case "parsed":
        // The whole point of the revision. A result older than one already
        // applied describes a document that no longer exists.
        if (response.rev < this.#applied) return;

        this.#applied = response.rev;
        this.desynced = null;
        this.chunks = response.result.chunks;
        this.diagnostics = response.result.diagnostics;
        return;
    }
  }

  /** Counts by severity, for the status bar. */
  get counts(): { error: number; warning: number; information: number } {
    const counts = { error: 0, warning: 0, information: 0 };
    for (const diagnostic of this.diagnostics) counts[diagnostic.severity] += 1;
    return counts;
  }
}

export const engine = new Engine();
