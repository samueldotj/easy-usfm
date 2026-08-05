/**
 * The main thread's half of the engine conversation.
 *
 * Holds `latestAppliedRev` and discards anything older (ARCHITECTURE §8.3).
 * The worker answers out of order under load, and a late reply carrying stale
 * offsets would light up diagnostics on text that has since been retyped.
 */

import { DeltaBuffer, type Batch } from "./delta";
import type {
  Diagnostic,
  Edit,
  ParseResult,
  Request,
  Response,
  Token,
} from "../worker/protocol";

export type { Diagnostic } from "../worker/protocol";

/** How long typing must stop before the mirror is verified. */
const IDLE_MS = 400;

/**
 * How long viewport changes are coalesced before asking for highlighting.
 *
 * Short enough not to be seen, long enough that a scroll produces one request
 * rather than one per frame.
 */
const TOKEN_MS = 30;

/**
 * The part of `Worker` this uses.
 *
 * Narrow on purpose: a fake in a test implements three members rather than
 * pretending to be a browser primitive.
 */
export interface WorkerLike {
  postMessage(message: unknown): void;
  terminate(): void;
  onmessage: ((event: { data: Response }) => void) | null;
}

/**
 * The real worker, behind {@link WorkerLike}.
 *
 * Adapted rather than assigned: `Worker.onmessage` takes a full `MessageEvent`,
 * which is wider than this needs, and a wider parameter type is not assignable
 * to a narrower one. Narrowing it here is honest about the one field that is
 * actually read, and keeps the fake in the tests from having to impersonate a
 * browser primitive.
 */
function adapt(): WorkerLike {
  const worker = new Worker(new URL("../worker/engine.worker.ts", import.meta.url), {
    type: "module",
    name: "easy-usfm-engine",
  });

  const like: WorkerLike = {
    postMessage: (message) => worker.postMessage(message),
    terminate: () => worker.terminate(),
    onmessage: null,
  };
  worker.onmessage = (event: MessageEvent<Response>) => like.onmessage?.({ data: event.data });
  return like;
}

export class Engine {
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

  #worker: WorkerLike | null = null;
  #rev = 0;
  #applied = 0;
  /** The last text sent, so a desync can be repaired without asking anyone. */
  #text = "";
  #buffer = new DeltaBuffer();
  #idleTimer: ReturnType<typeof setTimeout> | null = null;
  #tokenTimer: ReturnType<typeof setTimeout> | null = null;
  #wantedTokens: { from: number; to: number } | null = null;

  /**
   * Connects to the engine.
   *
   * `factory` exists so the protocol can be tested without a browser: the
   * behaviour worth testing here — discarding stale results, repairing a
   * desync — is all in how replies are handled, and none of it should require
   * spawning a real worker to exercise.
   */
  start(factory?: () => WorkerLike): void {
    if (this.#worker) return;

    const worker = factory?.() ?? adapt();
    this.#worker = worker;
    worker.onmessage = (event) => this.#receive(event.data);
    // Nothing is asked of the engine until it says it is ready. The WASM
    // module is fetched and instantiated asynchronously, so a request sent at
    // construction reaches a worker whose exports do not exist yet — it throws,
    // is caught as a desync, and the answer is simply lost.
  }

  stop(): void {
    if (this.#idleTimer !== null) clearTimeout(this.#idleTimer);
    if (this.#tokenTimer !== null) clearTimeout(this.#tokenTimer);
    this.#idleTimer = null;
    this.#tokenTimer = null;
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

    const batch = this.#buffer.push(edits, text);
    if (batch) this.#dispatch(batch);
    this.#scheduleIdle();
  }

  /**
   * Asks for highlighting over a range.
   *
   * Answers arrive at {@link ontokens}. Coalesced to one outstanding request:
   * scrolling produces a viewport update per frame, and asking the engine to
   * lex sixty times a second would put the cheap tier's work back on the
   * expensive path.
   */
  requestTokens(from: number, to: number): void {
    // Recorded even when the engine cannot answer yet. The first request
    // arrives while the WASM module is still instantiating — dropping it means
    // the document that is already on screen never gets highlighted, because
    // nothing will ask again until the user scrolls.
    this.#wantedTokens = { from, to };
    this.#pumpTokens();
  }

  #pumpTokens(): void {
    if (!this.ready || this.desynced || this.#tokenTimer !== null) return;
    if (!this.#wantedTokens) return;

    this.#tokenTimer = setTimeout(() => {
      this.#tokenTimer = null;
      const wanted = this.#wantedTokens;
      if (wanted) this.#send({ kind: "tokens", rev: this.#next(), ...wanted });
    }, TOKEN_MS);
  }

  /** Called with each answer. Set by the editor. */
  ontokens: ((from: number, to: number, tokens: Token[]) => void) | null = null;

  /** An input method has started. Nothing goes over until it commits. */
  startComposition(): void {
    this.#buffer.startComposition();
  }

  /** An input method has committed. One batch for the whole word. */
  endComposition(text: string): void {
    this.#text = text;
    const batch = this.#buffer.endComposition(text);
    if (batch) this.#dispatch(batch);
    this.#scheduleIdle();
  }

  #dispatch(batch: Batch): void {
    this.#send({
      kind: "edit",
      rev: this.#next(),
      edits: batch.edits,
      checksum: batch.checksum,
    });
  }

  /**
   * Verifies the mirror once typing stops.
   *
   * The cheap moment to check: nothing is competing for the main thread, and
   * drift caught here is caught before the next keystroke builds on it.
   */
  #scheduleIdle(): void {
    if (this.#idleTimer !== null) clearTimeout(this.#idleTimer);
    this.#idleTimer = setTimeout(() => {
      this.#idleTimer = null;
      if (this.#buffer.composing) return;

      const batch = this.#buffer.idle(this.#text);
      if (batch) this.#dispatch(batch);
    }, IDLE_MS);
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

      case "tokens":
        this.ontokens?.(response.from, response.to, response.tokens);
        return;

      case "parsed":
        // The whole point of the revision. A result older than one already
        // applied describes a document that no longer exists.
        if (response.rev < this.#applied) return;

        this.#applied = response.rev;
        this.desynced = null;
        this.chunks = response.result.chunks;
        this.diagnostics = response.result.diagnostics;
        // The document just changed on this side, so whatever highlighting was
        // asked for is now worth answering.
        this.#pumpTokens();
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
