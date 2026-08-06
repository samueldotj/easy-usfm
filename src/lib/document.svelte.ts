/**
 * The open document, and the file lifecycle around it.
 *
 * PRODUCT §3: one file per window. New, Open, Save, Save As.
 *
 * The fidelity envelope is *not* here. It lives in the shell, which is what
 * keeps the thing standing between a translator and a corrupted file out of
 * reach of the interface. What this holds is the text, whether it has been
 * edited, and enough of a summary to fill the status bar.
 */

import {
  documentService,
  type DocumentService,
  type FileChanged,
  type Opened,
  type Owner,
  type Reopen,
  type SaveOutcome,
  type Summary,
} from "./documentService";
import { LineTerminators, type Change } from "./eol";
import { read, write } from "./settings";

export type { Summary } from "./documentService";

const RECENT_KEY = "recent";
const RECENT_LIMIT = 10;

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((entry) => typeof entry === "string");

/**
 * A document, as the interface sees it.
 */
class DocumentState {
  id = $state<number | null>(null);

  /**
   * How many documents have been opened in this session.
   *
   * The identity of "the document that is open now", for anything that must
   * reset when it changes. Not `id`, which the desktop assigns and the browser
   * leaves `null` forever -- keying on that made the per-document figure
   * opt-in (SECURITY 3) reset on the desktop and never reset on the web, so
   * turning images on for one file quietly turned them on for the rest of the
   * session. Not `path` either: a new document has none, and two of them in a
   * row would look like the same document.
   */
  generation = $state(0);
  path = $state<string | null>(null);
  text = $state("");
  summary = $state<Summary | null>(null);

  /**
   * What this host cannot do, in words, for the interface to show.
   *
   * Empty on the desktop. On the web it is the difference between saving and
   * downloading, which the user has to know before they rely on it — an editor
   * that appears to save and does not is the worst failure available to it.
   */
  limitations = $state<readonly string[]>([]);

  /** Whether Save writes back to the file, or produces a copy. */
  savesInPlace = $state(true);

  /** Unsaved changes. The one piece of state a close must respect. */
  dirty = $state(false);

  /** Set when a save took the slower rung, so the delay has a visible reason. */
  saveNote = $state<string | null>(null);

  recent = $state<string[]>(read(RECENT_KEY, [], isStringArray));

  /** Per-line terminators, carried through edits. Never rebuilt from scratch. */
  #eols = new LineTerminators([], "lf");

  get name(): string {
    if (!this.path) return "Untitled";
    return this.path.split(/[/\\]/).pop() ?? this.path;
  }

  /** What the window title should say. */
  get title(): string {
    return `${this.dirty ? "• " : ""}${this.name} — Easy USFM`;
  }

  #adopt(opened: Opened): void {
    this.generation += 1;
    this.id = opened.id;
    this.path = opened.path;
    this.text = opened.text;
    this.summary = opened.summary;
    this.dirty = false;
    this.saveNote = null;
    this.#eols = new LineTerminators(opened.eols, opened.eols[0] ?? "lf");

    if (opened.path) this.#remember(opened.path);
  }

  #remember(path: string): void {
    this.recent = [path, ...this.recent.filter((entry) => entry !== path)].slice(
      0,
      RECENT_LIMIT,
    );
    write(RECENT_KEY, this.recent);
    void this.pushRecentToMenu();
  }

  /**
   * Pushes the recent list into the native menu.
   *
   * The list lives in settings, which the shell cannot read, so Open Recent
   * would otherwise be permanently empty. Called on startup and whenever the
   * list changes. The web service does nothing with it, having no menu.
   */
  async pushRecentToMenu(): Promise<void> {
    const service = await documentService();
    await service.setRecentFiles(this.recent);
  }

  /**
   * Writes a recovery snapshot of what is in the editor now.
   *
   * Silent on failure, deliberately. A snapshot is a safety net nobody asked
   * for; reporting that one could not be written would interrupt the typing it
   * exists to protect, with a message about a directory. FILE-FIDELITY 4's
   * guarantee is that recovery is *offered* when a snapshot exists, not that
   * one always does.
   */
  async snapshot(cursor: number): Promise<void> {
    try {
      const service = await documentService();
      await service.snapshot(this.#editable(), {
        cursor,
        dirty: this.dirty,
        // The per-line array is authoritative and already in the engine's own
        // spelling; the summary's `eol` is a label for the status bar.
        eol: this.#eols.dominant(),
        finalNewline: this.summary?.final_newline ?? true,
      });
    } catch {
      // See above.
    }
  }

  /**
   * Whether another instance holds this file, so the editor is read-only.
   *
   * FILE-FIDELITY 4: a live foreign process means "open read-only with Open a
   * copy or Take over". Nothing here enforces it at the filesystem -- the lock
   * is advisory -- so this is what the interface refuses on.
   */
  readOnly = $state(false);

  /** Who holds it, when someone else does. For the notice to name them. */
  heldBy = $state<Owner | null>(null);

  /**
   * Asks what is waiting for a file, before opening it.
   *
   * Returns the offer so the caller can prompt; taking the lock is separate,
   * because taking over from a live instance is a choice the user makes.
   */
  async examine(path: string): Promise<Reopen | null> {
    const service = await documentService();
    return service.examine(path);
  }

  /**
   * Gives the lock up, on a clean close or when moving to another file.
   *
   * Takes the path explicitly rather than reading `this.path`, because it is
   * called *after* the document has moved on -- the file being released is no
   * longer the one open.
   */
  async releaseLock(path: string): Promise<void> {
    try {
      const service = await documentService();
      await service.releaseLock(path);
    } catch {
      // A lock left behind reads as a crash next time, which offers a recovery
      // the user can decline. Not worth a dialog.
    }
  }

  /**
   * Detaches the buffer from the file it came from.
   *
   * FILE-FIDELITY 4's "Open a copy" for a file another instance holds. The text
   * stays and the path goes, which turns Save into Save As -- nothing is copied
   * on disk, because where the copy belongs is the user's decision. Dirty,
   * because this buffer now exists nowhere else.
   */
  detach(): void {
    this.path = null;
    this.dirty = true;
    this.id = null;
  }

  /** Watches the open file for changes made elsewhere (FILE-FIDELITY 3). */
  async watch(onchange: (change: FileChanged) => void): Promise<void> {
    if (!this.path) return;
    const service = await documentService();
    await service.watch(this.path, onchange);
  }

  async unwatch(): Promise<void> {
    const service = await documentService();
    await service.unwatch();
  }

  /**
   * Replaces the buffer with what is now on disk.
   *
   * Clean afterwards, unlike `restore`: the text and the file agree, which is
   * the whole point of reloading. The envelope is not re-read here -- the
   * shell recaptured it when the change was reported, and the terminators come
   * back uniform for the same reason a recovered buffer does.
   */
  reload(text: string): void {
    this.text = text;
    this.dirty = false;
    this.saveNote = null;
    this.#eols = LineTerminators.uniform(countNewlines(text), this.#eols.dominant());
  }

  /** Takes the lock and stops refusing edits. */
  async takeOver(path: string): Promise<void> {
    const service = await documentService();
    await service.takeLock(path);
    this.readOnly = false;
    this.heldBy = null;
  }

  /**
   * Replaces the buffer with a recovered snapshot.
   *
   * Dirty on purpose. The work is not on disk -- that is the whole reason it
   * was offered -- so the document has unsaved changes from the moment it is
   * restored, and the close warning has to fire for them.
   */
  restore(text: string): void {
    this.text = text;
    this.dirty = true;
    // Uniform, using whatever the file was opened as. The snapshot carries the
    // document's dominant terminator rather than a per-line array — a recovered
    // buffer has no history for the per-line map to have been carried through,
    // and inventing one would be worse than a consistent file.
    this.#eols = LineTerminators.uniform(countNewlines(text), this.#eols.dominant());
  }

  /** Forgets them, on a clean save or a clean close (FILE-FIDELITY 4). */
  async clearSnapshots(): Promise<void> {
    try {
      const service = await documentService();
      await service.clearSnapshots(this.#editable());
    } catch {
      // Leftover snapshots are offered back on the next launch, which is a
      // nuisance rather than a loss -- and not worth a dialog either.
    }
  }

  /**
   * The bytes of a figure this document asked for (SECURITY 3).
   *
   * Routed through the document because the document is what the request is
   * scoped to: the shell resolves the path against *this* file's folder, and
   * closing it is what ends the access. `null` where the host cannot load
   * local files, which is every browser.
   */
  async readFigure(path: string): Promise<Uint8Array | null> {
    const service = await documentService();
    return service.readFigure(this.id, path);
  }

  clearRecent(): void {
    this.recent = [];
    write(RECENT_KEY, this.recent);
    void this.pushRecentToMenu();
  }

  /** Records an edit. Called from the editor's update listener. */
  edited(text: string, changes: Change[]): void {
    this.text = text;
    this.dirty = true;
    this.saveNote = null;
    this.#eols = this.#eols.apply(text, changes);
  }

  async createNew(): Promise<void> {
    const service = await documentService();
    this.#adopt(await service.createNew());
    this.#refreshHostFacts(service);
  }

  /**
   * Opens a file the operating system handed over (P5.2).
   *
   * The same adoption an ordinary open performs, minus the picker: the user
   * already chose this file, by double-clicking it.
   */
  async adopt(handle: FileSystemFileHandle): Promise<void> {
    const service = await documentService();
    const opened = await service.adopt(handle);
    if (!opened) return;

    this.#adopt(opened);
    this.#refreshHostFacts(service);
  }

  async open(path?: string): Promise<void> {
    const service = await documentService();
    const opened = await service.open(path);
    // `null` is a cancelled dialog, which is not a failure and must not
    // disturb the document already open.
    if (!opened) return;

    this.#adopt(opened);
    this.#refreshHostFacts(service);
  }

  /**
   * Saves, asking for a location only when there is not one already.
   *
   * A read-only target is not reported as a failure: the answer is Save As
   * (FILE-FIDELITY §2, rung 3), and the service turns it into one.
   */
  async save(): Promise<boolean> {
    const service = await documentService();
    const outcome = service.canSaveInPlace(this.#editable())
      ? await service.save(this.#editable())
      : await service.saveAs(this.#editable());

    return this.#applyOutcome(outcome, service);
  }

  async saveAs(): Promise<boolean> {
    const service = await documentService();
    return this.#applyOutcome(await service.saveAs(this.#editable()), service);
  }

  /** What a service needs to write this document. */
  #editable() {
    return {
      id: this.id,
      path: this.path,
      text: this.text,
      eols: this.#eols.toArray(),
      bom: this.summary?.bom ?? false,
    };
  }

  #applyOutcome(outcome: SaveOutcome, service: DocumentService): boolean {
    // A cancelled dialog leaves everything exactly as it was, including the
    // dirty flag -- the work is still unsaved and the close warning must
    // still fire.
    if (!outcome.saved) return false;

    this.path = outcome.path;
    this.summary = outcome.summary;
    this.saveNote = outcome.note;
    this.dirty = false;
    this.#refreshHostFacts(service);

    if (outcome.path) this.#remember(outcome.path);

    // FILE-FIDELITY 4: "cleared on clean save". The file on disk now holds the
    // work, so a snapshot of it is no longer a safety net -- it is an offer to
    // restore something the user already has, which on the next launch reads
    // as the application having lost their save.
    void this.clearSnapshots();
    return true;
  }

  /**
   * Re-reads what the host can do.
   *
   * Not fixed at startup, because on the web it changes: a document opened
   * through a file input cannot be saved back to, and one opened through the
   * File System Access API can. The status bar has to follow.
   */
  #refreshHostFacts(service: DocumentService): void {
    this.limitations = service.limitations;
    this.savesInPlace = service.canSaveInPlace(this.#editable());
  }

  async confirmDiscard(): Promise<boolean> {
    if (!this.dirty) return true;
    const service = await documentService();
    return service.confirmDiscard(this.name);
  }
}

export const doc = new DocumentState();

/** How many line terminators a recovered buffer needs. */
function countNewlines(text: string): number {
  let count = 0;
  for (const character of text) {
    if (character === "\n") count += 1;
  }
  return count;
}
