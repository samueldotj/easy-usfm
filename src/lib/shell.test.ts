/**
 * The host boundary, and the one trap in it.
 *
 * `confirmed` exists because `window.confirm` is not the same function on the
 * two hosts: in a browser it is the browser's and returns a boolean, and on
 * the desktop Tauri has replaced it with an async one that returns a promise.
 * Both shapes have to come back as a boolean, and the desktop one has to be
 * waited for — a promise read as a boolean is always truthy, which is the bug
 * this file is here to stop coming back.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import { confirmed } from "./shell";

const original = globalThis.confirm;

afterEach(() => {
  globalThis.confirm = original;
  vi.restoreAllMocks();
});

/** Replaces the global with whatever a host would have put there. */
function hostAnswers(answer: boolean | Promise<boolean>): void {
  globalThis.confirm = (() => answer) as typeof globalThis.confirm;
}

describe("asking the user", () => {
  it("reads a browser's synchronous answer", async () => {
    hostAnswers(true);
    await expect(confirmed("sure?")).resolves.toBe(true);

    hostAnswers(false);
    await expect(confirmed("sure?")).resolves.toBe(false);
  });

  it("waits for the desktop's asynchronous one", async () => {
    // Tauri's replacement is `async function`, so it answers with a promise.
    // Read without awaiting, a promise is an object and every object is
    // truthy — which turned "no" into "yes" at every call site.
    hostAnswers(Promise.resolve(false));
    await expect(confirmed("sure?")).resolves.toBe(false);

    hostAnswers(Promise.resolve(true));
    await expect(confirmed("sure?")).resolves.toBe(true);
  });

  it("is the question the caller asked", async () => {
    const asked = vi.fn(() => true);
    globalThis.confirm = asked as unknown as typeof globalThis.confirm;

    await confirmed("Remove this sidebar?");

    expect(asked).toHaveBeenCalledWith("Remove this sidebar?");
  });

  it("answers a promise, so a caller who forgets to wait is a type error", () => {
    hostAnswers(true);
    // Not an assertion about the value: the point is the static type, which
    // is what makes the mistake impossible to repeat silently.
    expect(confirmed("sure?")).toBeInstanceOf(Promise);
  });
});
