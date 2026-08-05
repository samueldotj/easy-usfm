/// <reference lib="webworker" />

/**
 * The engine, in the worker it runs in on every target.
 *
 * ARCHITECTURE §11: "the engine runs in a dedicated Web Worker on both targets;
 * the main thread never parses." That is what makes *typing is never blocked
 * by parsing* a structural property rather than a hope — there is no code path
 * on the main thread that could block it, however large the document gets.
 *
 * The module is loaded here and nowhere else. Nothing on the main thread
 * imports the WASM.
 */

import init, { Session, version } from "../generated/wasm/easy_usfm_wasm";
import wasmUrl from "../generated/wasm/easy_usfm_wasm_bg.wasm?url";
import type {
  Completion,
  Match,
  ParseResult,
  Request,
  Resolution,
  Response,
  Token,
} from "./protocol";

let session: Session | null = null;

function reply(message: Response): void {
  self.postMessage(message);
}

function desync(rev: number, reason: string): void {
  session = null;
  reply({ kind: "desync", rev, reason });
}

/** The error a refused edit carries back from the engine. */
function reasonOf(error: unknown): string {
  if (error && typeof error === "object" && "error" in error) {
    return String((error as { error: unknown }).error);
  }
  return error instanceof Error ? error.message : String(error);
}

async function ready(): Promise<void> {
  // Explicit URL rather than the default relative guess: the bundler rewrites
  // the asset path, and letting wasm-bindgen resolve it itself works in dev
  // and silently 404s in a build.
  await init({ module_or_path: wasmUrl });
  reply({ kind: "ready" });
}

self.onmessage = (event: MessageEvent<Request>) => {
  const request = event.data;

  try {
    switch (request.kind) {
      case "version":
        reply({ kind: "version", rev: request.rev, version: version() });
        break;

      case "tokens": {
        if (!session) return;
        reply({
          kind: "tokens",
          rev: request.rev,
          from: request.from,
          to: request.to,
          tokens: session.tokens(request.from, request.to) as Token[],
        });
        break;
      }

      case "resolve": {
        if (!session) return;
        reply({
          kind: "resolved",
          rev: request.rev,
          result: session.resolve(request.text) as Resolution,
        });
        break;
      }

      case "find": {
        if (!session) return;
        reply({
          kind: "found",
          rev: request.rev,
          matches: session.find(request.query, request.exact) as Match[],
        });
        break;
      }

      case "completions": {
        if (!session) return;
        reply({
          kind: "completions",
          rev: request.rev,
          completions: session.completions(request.at) as Completion[],
        });
        break;
      }

      case "where": {
        if (!session) return;
        reply({
          kind: "where",
          rev: request.rev,
          reference: session.referenceAt(request.at) ?? null,
        });
        break;
      }

      case "override-version": {
        if (!session) return;
        reply({
          kind: "parsed",
          rev: request.rev,
          result: session.overrideVersion(request.version ?? undefined) as ParseResult,
        });
        break;
      }

      case "open":
      case "resync":
        session?.free();
        session = new Session(request.text);
        reply({
          kind: "parsed",
          rev: request.rev,
          result: session.snapshot() as ParseResult,
        });
        break;

      case "edit": {
        if (!session) {
          desync(request.rev, "no document is open");
          return;
        }

        // Applied in order. Each edit's offsets are against the document as
        // the engine currently holds it, which is what the main thread sends;
        // translating a batch stated in original-document coordinates is
        // P2.2's job, not the worker's.
        let result: ParseResult | null = null;
        for (const edit of request.edits) {
          result = session.edit(edit.from, edit.to, edit.insert) as ParseResult;
        }

        // Verified *after* applying, against the text the editor had when it
        // sent the batch. A mismatch means the two sides no longer hold the
        // same document, and every offset in the reply would be wrong.
        if (request.checksum !== undefined && session.checksum !== request.checksum) {
          desync(
            request.rev,
            `mirror checksum ${session.checksum} does not match the editor's ${request.checksum}`,
          );
          return;
        }

        reply({
          kind: "parsed",
          rev: request.rev,
          result: result ?? (session.snapshot() as ParseResult),
        });
        break;
      }
    }
  } catch (error) {
    // A refused edit means this side and the editor no longer agree. Carrying
    // on would produce offsets that point at text the user never wrote.
    desync(request.rev, reasonOf(error));
  }
};

void ready();
