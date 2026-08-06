/**
 * The main thread's half of the engine conversation.
 *
 * Holds `latestAppliedRev` and discards anything older (ARCHITECTURE §8.3).
 * The worker answers out of order under load, and a late reply carrying stale
 * offsets would light up diagnostics on text that has since been retyped.
 */

import { DeltaBuffer, type Batch } from "./delta";
import type {
  Completion,
  Diagnostic,
  Match,
  Edit,
  ParseResult,
  PreviewNode,
  Request,
  Response,
  Resolution,
  Token,
  UsfmVersion,
} from "../worker/protocol";

export type {
  Chunk,
  Completion,
  Diagnostic,
  Match,
  PreviewNode,
  Resolution,
  UsfmVersion,
} from "../worker/protocol";

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
 * How long the cursor must settle before the status bar asks where it is.
 *
 * Arrow-keying through a verse would otherwise be one round trip per
 * character, to answer a question whose answer only changes at verse
 * boundaries.
 */
const WHERE_MS = 120;

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
   * The rendered nodes, per chunk.
   *
   * Keyed by chunk index, which is also what the preview iterates. Held here
   * rather than fetched by the component so that a chapter scrolling back into
   * view is instant -- and so the request is made once per *change*, not once
   * per mount.
   */
  previews = $state<(PreviewNode[] | undefined)[]>([]);

  /** Where the cursor is, as a reference. `null` before the first verse. */
  reference = $state<string | null>(null);

  /** The *document's* USFM version, which is not the engine's. */
  usfm = $state<UsfmVersion>({
    declared: null,
    effective: "3.0",
    overridden: false,
    assumed: "3.0",
  });

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
  /**
   * The user's version choice, held here rather than in the engine.
   *
   * A desync frees the worker's session, so the engine cannot be the authority
   * on anything that has to survive one. This side already resends the text
   * for exactly that reason; the override goes with it.
   */
  #override: string | null = null;
  #buffer = new DeltaBuffer();
  #idleTimer: ReturnType<typeof setTimeout> | null = null;
  #tokenTimer: ReturnType<typeof setTimeout> | null = null;
  #wantedTokens: { from: number; to: number } | null = null;
  #whereTimer: ReturnType<typeof setTimeout> | null = null;
  /** Resolutions in flight, by the revision that asked. */
  #pending = new Map<number, (result: Resolution) => void>();
  /** Completion requests in flight, likewise. */
  #asking = new Map<number, (offers: Completion[]) => void>();
  /** Searches in flight, likewise. */
  #searching = new Map<number, (matches: Match[]) => void>();
  /** Chapters whose rendering has been asked for and not yet arrived. */
  #requested = new Set<number>();

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
    if (this.#whereTimer !== null) clearTimeout(this.#whereTimer);
    this.#idleTimer = null;
    this.#tokenTimer = null;
    this.#whereTimer = null;
    this.#abandon("The engine has stopped.");
    this.#worker?.terminate();
    this.#worker = null;
    this.ready = false;
  }

  /**
   * Judges the document as a different USFM version.
   *
   * `null` returns to what the file declares. Nothing is written to the file —
   * PRODUCT §4 is explicit that the detected version is never written in
   * automatically, and an override that edited the header would dirty a
   * document the user only wanted to look at differently.
   */
  overrideVersion(version: string | null): void {
    this.#override = version;
    if (this.ready) this.#send({ kind: "override-version", rev: this.#next(), version });
  }

  open(text: string): void {
    this.#text = text;
    // A new document is judged on its own terms. Carrying the last one's
    // override across would silently apply a decision about one file to
    // another, and there is nothing on screen that would say so.
    this.#override = null;
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

  /**
   * Looks up a reference the user typed (PRODUCT §6.2).
   *
   * Resolved in the engine rather than here because it needs the verse index,
   * which is built from the parse — and because the `\vp` fallback compares
   * published numbers that only the engine has.
   */
  resolve(text: string): Promise<Resolution> {
    return new Promise((settle) => {
      if (!this.ready) {
        settle({ start: null, end: null, message: "The engine is still starting." });
        return;
      }
      const rev = this.#next();
      this.#pending.set(rev, settle);
      this.#send({ kind: "resolve", rev, text });
    });
  }

  /**
   * Asks for a chapter's nodes.
   *
   * Answers land in {@link previews}. Requested per chunk and only when that
   * chunk's revision has moved, which is the whole point of chunking: typing
   * in chapter forty must not re-render chapter one (ARCHITECTURE 10).
   */
  requestPreview(chunk: number): void {
    if (!this.ready) return;
    // One request per chapter in flight. The preview asks on every scroll and
    // on every parse, and without this a chapter near the viewport edge is
    // requested dozens of times while it sits there.
    if (this.previews[chunk] || this.#requested.has(chunk)) return;

    this.#requested.add(chunk);
    this.#send({ kind: "preview", rev: this.#next(), chunk });
  }

  /**
   * Every match for `query`.
   *
   * Positions only. The replacement is applied by the editor, because the
   * buffer is authoritative (ADR-003) and the delta protocol carries the edit
   * back here like any other — an engine that rewrote the document itself
   * would be a second writer to it.
   */
  find(query: string, exact: boolean): Promise<Match[]> {
    return new Promise((settle) => {
      if (!this.ready || query === "") {
        settle([]);
        return;
      }
      const rev = this.#next();
      this.#searching.set(rev, settle);
      this.#send({ kind: "find", rev, query, exact });
    });
  }

  /**
   * The marker list for a backslash at `at`.
   *
   * The engine ranks; the editor filters as the name is typed. Ranking needs
   * the marker table, the parse tree at that position, and a count over the
   * whole document -- none of which is on this side.
   */
  completions(at: number): Promise<Completion[]> {
    return new Promise((settle) => {
      if (!this.ready) {
        settle([]);
        return;
      }
      const rev = this.#next();
      this.#asking.set(rev, settle);
      this.#send({ kind: "completions", rev, at });
    });
  }

  /**
   * The cursor has moved; ask what reference it is at.
   *
   * Debounced and coalesced to one outstanding question. The answer only
   * changes at verse boundaries, so asking per keystroke would be a round trip
   * per character to learn nothing.
   */
  locate(at: number): void {
    if (!this.ready) return;
    if (this.#whereTimer !== null) clearTimeout(this.#whereTimer);

    this.#whereTimer = setTimeout(() => {
      this.#whereTimer = null;
      this.#send({ kind: "where", rev: this.#next(), at });
    }, WHERE_MS);
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
    // The engine has just been handed a fresh session, which knows nothing
    // about a choice made before it existed.
    if (this.#override !== null) {
      this.#send({ kind: "override-version", rev: this.#next(), version: this.#override });
    }
  }

  /**
   * Settles every waiting lookup with a failure.
   *
   * Called wherever the engine can no longer answer. Settling rather than
   * rejecting: a caller awaiting a reference is showing a dialog, and a
   * rejection there is an unhandled error rather than a message.
   */
  #abandon(message: string): void {
    for (const settle of this.#pending.values()) {
      settle({ start: null, end: null, message });
    }
    this.#pending.clear();

    // A completion list has nowhere to put a message, so an empty list is the
    // whole of what it can say. The popup simply does not appear.
    for (const settle of this.#asking.values()) settle([]);
    this.#asking.clear();
    for (const settle of this.#searching.values()) settle([]);
    this.#searching.clear();
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
        // Anything waiting for an answer will never get one -- the session
        // that was going to answer has been freed. A promise left unsettled
        // here is not a lost reply, it is a caller stuck forever, which is how
        // one bad lookup disables Go to Reference for the rest of the session.
        this.#abandon("The engine lost track of the document; try again.");
        // Repair immediately rather than waiting for the next keystroke: a
        // desynced engine shows stale diagnostics until it is fixed.
        this.#resync();
        return;

      case "tokens":
        this.ontokens?.(response.from, response.to, response.tokens);
        return;

      case "resolved": {
        // Settled by the revision that asked, so two lookups in flight cannot
        // answer each other.
        const settle = this.#pending.get(response.rev);
        this.#pending.delete(response.rev);
        settle?.(response.result);
        return;
      }

      case "completions": {
        const settle = this.#asking.get(response.rev);
        this.#asking.delete(response.rev);
        settle?.(response.completions);
        return;
      }

      case "previewed": {
        this.#requested.delete(response.chunk);
        // A chunk that has since been merged away by a `\c` edit is simply
        // dropped: the array is indexed by chunk, and writing past its end
        // would resurrect a chapter that no longer exists.
        if (response.chunk < this.chunks.length) {
          const next = [...this.previews];
          next[response.chunk] = response.nodes;
          this.previews = next;
        }
        return;
      }

      case "found": {
        const settle = this.#searching.get(response.rev);
        this.#searching.delete(response.rev);
        settle?.(response.matches);
        return;
      }

      case "where":
        this.reference = response.reference;
        return;

      case "parsed":
        // The whole point of the revision. A result older than one already
        // applied describes a document that no longer exists.
        if (response.rev < this.#applied) return;

        this.#applied = response.rev;
        this.desynced = null;
        this.#refreshPreviews(response.result.chunks);
        this.chunks = response.result.chunks;
        this.diagnostics = response.result.diagnostics;
        this.usfm = response.result.version;
        // The document just changed on this side, so whatever highlighting was
        // asked for is now worth answering.
        this.#pumpTokens();
        return;
    }
  }

  /**
   * Drops the rendering of every chapter whose revision moved.
   *
   * Dropped rather than re-requested. Asking for all of them here would mean
   * fifty round trips the moment a fifty-chapter document opens, to render
   * forty-nine chapters nobody has scrolled to -- which is the cost
   * ARCHITECTURE 10's overscan exists to avoid. The preview asks for what it
   * is about to show; this only says what is no longer true.
   *
   * Compared against the chunks held *before* this result, so an edit that
   * touched one chapter drops one rendering. A split or merge shifts the
   * chunks after it and their indices then name different chapters, so those
   * are dropped too -- correct rather than wasteful.
   */
  #refreshPreviews(next: ParseResult["chunks"]): void {
    const previous = this.chunks;
    const kept: (PreviewNode[] | undefined)[] = [];

    next.forEach((chunk, index) => {
      const before = previous[index];
      if (before && before.rev === chunk.rev && before.number === chunk.number) {
        kept[index] = this.previews[index];
      }
    });

    this.previews = kept;
    // A request already in flight names a chapter this may have just changed.
    // Its answer would overwrite the drop with what the chapter used to be.
    this.#requested.clear();
  }

  /** Counts by severity, for the status bar. */
  get counts(): { error: number; warning: number; information: number } {
    const counts = { error: 0, warning: 0, information: 0 };
    for (const diagnostic of this.diagnostics) counts[diagnostic.severity] += 1;
    return counts;
  }
}

export const engine = new Engine();
