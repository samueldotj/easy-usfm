/**
 * Whether images are shown, and the ones that have been loaded — SECURITY §3.
 *
 * "Images are off by default, with a per-document opt-in enabling local files
 * only."
 *
 * # Per document, and not remembered
 *
 * The opt-in resets whenever the document changes. That is the point of it
 * being per document rather than a setting: a decision to trust one file's
 * images is not a decision to trust the next file's, and a preference that
 * persists would quietly turn "off by default" into "on from now on". Someone
 * who opens a file from a stranger gets the same starting position as they did
 * the first time.
 *
 * # Blob URLs, and why they are revoked
 *
 * Bytes arrive from the shell and become object URLs, which is what an `<img>`
 * can take. Each one pins its data in memory until revoked, so a document with
 * a hundred figures browsed for an hour would hold all of them — the revocation
 * on reset is what keeps that from being a leak that only shows up on a long
 * session with a large book.
 */

/** Whatever can fetch a figure's bytes -- the open document, in practice. */
export interface Source {
  readFigure(path: string): Promise<Uint8Array | null>;
}

/** What became of one figure's `file` attribute. */
export type Loaded =
  | { state: "loading" }
  | { state: "ready"; url: string }
  | { state: "refused"; reason: string };

export class Figures {
  /**
   * The per-document opt-in. Off is the default and the reset value.
   */
  shown = $state(false);

  /** What has been asked for, by the path the `\fig` carried. */
  #loaded = $state<Record<string, Loaded>>({});

  /**
   * Which document the current state belongs to.
   *
   * Compared rather than trusted: `reset` is called from an effect, and an
   * effect that runs twice for the same document must not throw away images it
   * has already loaded.
   *
   * `-1` because no document has that generation, so the first reset always
   * takes -- and because a nullable identifier is what got this wrong before.
   */
  #for = -1;

  /** What is known about a figure, or `undefined` if it has not been asked for. */
  status(path: string): Loaded | undefined {
    return this.#loaded[path];
  }

  /** Turns the opt-in on or off for the document that is open. */
  toggle(on: boolean): void {
    this.shown = on;
    // Not revoked when switching off. Turning images off and on again is a
    // thing people do while reading, and re-reading every file from disk to
    // show the same pictures would make the toggle feel broken.
  }

  /**
   * Forgets everything, for a document that is no longer the one open.
   *
   * Called when the document changes rather than when it closes, because the
   * interface holds one document at a time and "changed" is the event it can
   * actually observe.
   */
  reset(document: number): void {
    if (this.#for === document) return;
    this.#for = document;

    for (const entry of Object.values(this.#loaded)) {
      if (entry.state === "ready") URL.revokeObjectURL(entry.url);
    }
    this.#loaded = {};
    this.shown = false;
  }

  /**
   * Asks the shell for one figure's bytes, once.
   *
   * Once per path per document: a chapter re-renders on every keystroke, and a
   * figure whose file has not changed should not be read from disk again for
   * each of them. The `loading` entry is what makes the second caller a no-op
   * before the first has answered.
   */
  async request(source: Source, path: string): Promise<void> {
    if (!this.shown || this.#loaded[path]) return;
    this.#loaded = { ...this.#loaded, [path]: { state: "loading" } };

    const belongsTo = this.#for;
    try {
      const bytes = await source.readFigure(path);
      // The document changed while this was in flight. Its bytes belong to a
      // file that is no longer open, and writing them into the map would show
      // one document's picture inside another.
      if (belongsTo !== this.#for) return;

      if (bytes === null) {
        this.#record(path, { state: "refused", reason: "not available here" });
        return;
      }

      // No sniffing and no declared type: the browser decides what the bytes
      // are, and an image that is not one simply fails to decode. Naming a
      // type here would be this side asserting something about content it has
      // not looked at.
      const url = URL.createObjectURL(new Blob([bytes as BlobPart]));
      this.#record(path, { state: "ready", url });
    } catch (error) {
      if (belongsTo !== this.#for) return;
      this.#record(path, { state: "refused", reason: message(error) });
    }
  }

  #record(path: string, entry: Loaded): void {
    const previous = this.#loaded[path];
    if (previous?.state === "ready") URL.revokeObjectURL(previous.url);
    this.#loaded = { ...this.#loaded, [path]: entry };
  }
}

function message(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "could not be read";
}

export const figures = new Figures();
