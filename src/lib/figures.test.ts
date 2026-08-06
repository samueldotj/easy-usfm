/**
 * The figure opt-in, and the two things about it that are security properties
 * rather than conveniences — SECURITY §3.
 *
 * "Images are off by default, with a per-document opt-in." Both halves are
 * testable and both have already been wrong once: the reset was keyed on an
 * identifier the browser leaves `null` forever, so turning images on for one
 * file turned them on for every file after it, on the web only. That is the
 * shape of bug this file exists to catch — the difference between per document
 * and per session is invisible until you open a second document.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";

import { Figures, type Source } from "./figures.svelte";

/** A host that answers with bytes, and counts how often it is asked. */
function source(answer: () => Promise<Uint8Array | null>): Source & { calls: number } {
  const stub = {
    calls: 0,
    async readFigure(): Promise<Uint8Array | null> {
      stub.calls += 1;
      return answer();
    },
  };
  return stub;
}

const bytes = () => Promise.resolve(new Uint8Array([1, 2, 3]));

beforeEach(() => {
  // Object URLs do not exist outside a browser, and what matters here is that
  // they are created and revoked in pairs -- so they are counted rather than
  // resolved.
  let next = 0;
  vi.stubGlobal("URL", {
    createObjectURL: vi.fn(() => `blob:${(next += 1)}`),
    revokeObjectURL: vi.fn(),
  });
  vi.stubGlobal("Blob", class {});
});

describe("the opt-in", () => {
  it("starts off", () => {
    expect(new Figures().shown).toBe(false);
  });

  it("asks for nothing while it is off", async () => {
    const figures = new Figures();
    const host = source(bytes);

    await figures.request(host, "art/map.png");

    // Not "returns nothing" -- *asks* nothing. An image that is off must not
    // reach the filesystem at all.
    expect(host.calls).toBe(0);
    expect(figures.status("art/map.png")).toBeUndefined();
  });

  it("goes back off for a different document", async () => {
    const figures = new Figures();
    figures.reset(1);
    figures.toggle(true);
    await figures.request(source(bytes), "art/map.png");

    figures.reset(2);

    expect(figures.shown).toBe(false);
    expect(figures.status("art/map.png")).toBeUndefined();
  });

  it("stays on through a reset for the same document", async () => {
    // `reset` is called from an effect, which can run more than once for the
    // same document; throwing away loaded images there would make figures
    // flicker for no reason.
    const figures = new Figures();
    figures.reset(1);
    figures.toggle(true);
    await figures.request(source(bytes), "art/map.png");

    figures.reset(1);

    expect(figures.shown).toBe(true);
    expect(figures.status("art/map.png")).toEqual({ state: "ready", url: "blob:1" });
  });
});

describe("loading", () => {
  it("asks once per figure however often it is rendered", async () => {
    const figures = new Figures();
    figures.reset(1);
    figures.toggle(true);
    const host = source(bytes);

    // A chapter re-renders on every keystroke; the file must not be re-read
    // for each of them.
    await figures.request(host, "art/map.png");
    await figures.request(host, "art/map.png");
    await figures.request(host, "art/map.png");

    expect(host.calls).toBe(1);
  });

  it("reports a refusal in words rather than as a missing image", async () => {
    const figures = new Figures();
    figures.reset(1);
    figures.toggle(true);

    await figures.request(
      source(() => Promise.reject(new Error("not a local file in the document's folder"))),
      "../secret.png",
    );

    expect(figures.status("../secret.png")).toEqual({
      state: "refused",
      reason: "not a local file in the document's folder",
    });
  });

  it("says so when the host cannot load local files at all", async () => {
    const figures = new Figures();
    figures.reset(1);
    figures.toggle(true);

    await figures.request(source(() => Promise.resolve(null)), "art/map.png");

    expect(figures.status("art/map.png")).toEqual({
      state: "refused",
      reason: "not available here",
    });
  });

  it("drops an answer that arrives after the document changed", async () => {
    const figures = new Figures();
    figures.reset(1);
    figures.toggle(true);

    let release: (value: Uint8Array) => void = () => {};
    const pending = figures.request(
      source(() => new Promise<Uint8Array>((resolve) => (release = resolve))),
      "art/map.png",
    );

    // The document closes while the read is in flight. Its bytes belong to a
    // file that is no longer open.
    figures.reset(2);
    release(new Uint8Array([9]));
    await pending;

    expect(figures.status("art/map.png")).toBeUndefined();
    expect(figures.shown).toBe(false);
  });

  it("revokes every object URL it hands out", async () => {
    const figures = new Figures();
    figures.reset(1);
    figures.toggle(true);

    await figures.request(source(bytes), "a.png");
    await figures.request(source(bytes), "b.png");
    figures.reset(2);

    // Each one pins its bytes in memory until revoked, so a long session over
    // a large book would hold all of them.
    expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:1");
    expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:2");
  });
});
