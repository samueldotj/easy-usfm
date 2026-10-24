/**
 * Which shape the window is in.
 *
 * The redesign offers two, and they are not two skins of one thing — they
 * answer different questions and a translator moves between them during a
 * session:
 *
 * - **Workbench** is for building a file. Four ruled columns: what is in the
 *   book, the source, the reading, and an inspector for whatever the caret is
 *   in — with diagnostics docked underneath where they can be worked through.
 * - **Study** is for reading what has been built. The navigator shrinks to a
 *   rail of chapter numbers, the reading column takes the space that buys, and
 *   the inspectors stop being columns and start being things that appear next
 *   to what they describe.
 *
 * Both are the same document, the same engine and the same panes; what differs
 * is how much room each one is given and whether the apparatus is docked or
 * floating.
 *
 * # Everything here is remembered
 *
 * A layout is a working habit, not a per-document property. Someone who works
 * in Study does not want Workbench back every morning — so all of it goes
 * through `settings`, and none of it is keyed on the file.
 */

import { read, write } from "./settings";

/** The two arrangements. */
export type Shell = "workbench" | "study";

/** Which panes are on screen. `split` is both, which is the point of the app. */
export type Panes = "editor" | "split" | "preview";

/** Which inspector the side panel is showing (Workbench), or floating (Study). */
export type Inspector = "sidebars" | "footnote" | "marker";

const SHELLS: Shell[] = ["workbench", "study"];
const PANES: Panes[] = ["editor", "split", "preview"];
const INSPECTORS: Inspector[] = ["sidebars", "footnote", "marker"];

const isShell = (value: unknown): value is Shell =>
  typeof value === "string" && (SHELLS as string[]).includes(value);
const isPanes = (value: unknown): value is Panes =>
  typeof value === "string" && (PANES as string[]).includes(value);
const isInspector = (value: unknown): value is Inspector =>
  typeof value === "string" && (INSPECTORS as string[]).includes(value);
const isBoolean = (value: unknown): value is boolean => typeof value === "boolean";

class LayoutState {
  shell = $state<Shell>(read("layout.shell", "workbench", isShell));
  panes = $state<Panes>(read("layout.panes", "split", isPanes));
  inspector = $state<Inspector>(read("layout.inspector", "sidebars", isInspector));

  /**
   * Whether the two panes follow each other (P3.6).
   *
   * A setting rather than always-on, because there is one case where it is
   * exactly wrong: comparing a passage against a distant one, which is a thing
   * translators do constantly and which sync scroll makes impossible.
   */
  sync = $state<boolean>(read("layout.sync", true, isBoolean));

  /** The navigator column, which Workbench can fold away to reclaim its 208px. */
  navigator = $state<boolean>(read("layout.navigator", true, isBoolean));

  /** The docked diagnostics table. */
  diagnostics = $state<boolean>(read("layout.diagnostics", true, isBoolean));

  setShell(shell: Shell): void {
    this.shell = shell;
    write("layout.shell", shell);
  }

  setPanes(panes: Panes): void {
    this.panes = panes;
    write("layout.panes", panes);
  }

  setInspector(inspector: Inspector): void {
    this.inspector = inspector;
    write("layout.inspector", inspector);
  }

  toggleSync(): void {
    this.sync = !this.sync;
    write("layout.sync", this.sync);
  }

  toggleNavigator(): void {
    this.navigator = !this.navigator;
    write("layout.navigator", this.navigator);
  }

  toggleDiagnostics(): void {
    this.diagnostics = !this.diagnostics;
    write("layout.diagnostics", this.diagnostics);
  }

  /** Whichever pane is not showing is not somewhere focus or a scroll can go. */
  get showsEditor(): boolean {
    return this.panes !== "preview";
  }

  get showsPreview(): boolean {
    return this.panes !== "editor";
  }
}

export const layout = new LayoutState();
