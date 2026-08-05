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

import { LineTerminators, type Change, type Eol } from "./eol";
import { isDesktop } from "./shell";
import { read, write } from "./settings";

export interface Summary {
  encoding: string;
  eol: string;
  bom: boolean;
  final_newline: boolean;
  mixed_eol: boolean;
}

interface Opened {
  id: number;
  path: string | null;
  text: string;
  summary: Summary;
  eols: Eol[];
}

interface SaveReport {
  path: string;
  reason: string | null;
  summary: Summary;
}

const RECENT_KEY = "recent";
const RECENT_LIMIT = 10;

const isStringArray = (value: unknown): value is string[] =>
  Array.isArray(value) && value.every((entry) => typeof entry === "string");

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: call } = await import("@tauri-apps/api/core");
  return call<T>(command, args);
}

/**
 * A document, as the interface sees it.
 */
class DocumentState {
  id = $state<number | null>(null);
  path = $state<string | null>(null);
  text = $state("");
  summary = $state<Summary | null>(null);

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
   * list changes.
   */
  async pushRecentToMenu(): Promise<void> {
    if (!isDesktop()) return;
    try {
      await invoke("set_recent_files", { paths: this.recent });
    } catch {
      // A menu that failed to rebuild is not worth interrupting anyone over;
      // every command in it is still reachable by its accelerator.
    }
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
    if (!isDesktop()) {
      this.text = "\\id XXA\n\\h \n\\mt1 \n\\c 1\n\\p\n\\v 1 \n";
      this.dirty = false;
      return;
    }
    this.#adopt(await invoke<Opened>("new_document"));
  }

  async open(path?: string): Promise<void> {
    if (!isDesktop()) throw new Error("opening files needs the desktop application");

    let chosen = path;
    if (!chosen) {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "USFM", extensions: ["usfm", "sfm", "SFM", "USFM"] }],
      });
      if (typeof picked !== "string") return;
      chosen = picked;
    }

    this.#adopt(await invoke<Opened>("open_document", { path: chosen }));
  }

  /**
   * Saves, asking for a path only when there is not one already.
   *
   * A read-only target is not reported as a failure: the shell marks it, and
   * the answer is Save As (FILE-FIDELITY §2, rung 3).
   */
  async save(): Promise<boolean> {
    if (this.id === null) return false;
    if (!this.path) return this.saveAs();

    try {
      this.#applyReport(
        await invoke<SaveReport>("save_document", {
          request: { id: this.id, text: this.text, eols: this.#eols.toArray() },
          path: null,
        }),
      );
      return true;
    } catch (error) {
      const message = String(error);
      if (message.includes("READONLY:")) return this.saveAs();
      throw error;
    }
  }

  async saveAs(): Promise<boolean> {
    if (this.id === null) return false;

    const { save } = await import("@tauri-apps/plugin-dialog");
    const chosen = await save({
      defaultPath: this.path ?? "untitled.usfm",
      filters: [{ name: "USFM", extensions: ["usfm", "sfm"] }],
    });
    if (typeof chosen !== "string") return false;

    this.#applyReport(
      await invoke<SaveReport>("save_document", {
        request: { id: this.id, text: this.text, eols: this.#eols.toArray() },
        path: chosen,
      }),
    );
    return true;
  }

  #applyReport(report: SaveReport): void {
    this.path = report.path;
    this.summary = report.summary;
    this.saveNote = report.reason;
    this.dirty = false;
    this.#remember(report.path);
  }

  /**
   * Whether it is safe to discard this document.
   *
   * Asked before closing and before opening another file. The dialog is
   * native, and its default is the safe answer.
   */
  async confirmDiscard(): Promise<boolean> {
    if (!this.dirty) return true;
    if (!isDesktop()) return confirm(`${this.name} has unsaved changes. Discard them?`);

    const { ask } = await import("@tauri-apps/plugin-dialog");
    return ask(`${this.name} has unsaved changes. Discard them?`, {
      title: "Unsaved changes",
      kind: "warning",
      okLabel: "Discard",
      cancelLabel: "Cancel",
    });
  }
}

export const doc = new DocumentState();
