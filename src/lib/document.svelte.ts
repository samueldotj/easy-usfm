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
  type Opened,
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
