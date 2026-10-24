<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  import AppBar from "./components/AppBar.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import DiagnosticsPanel from "./components/DiagnosticsPanel.svelte";
  import DocumentBar from "./components/DocumentBar.svelte";
  import Editor from "./components/Editor.svelte";
  import ExternalChange from "./components/ExternalChange.svelte";
  import FindBar from "./components/FindBar.svelte";
  import FloatingNote from "./components/FloatingNote.svelte";
  import FontNotice from "./components/FontNotice.svelte";
  import GoToReference from "./components/GoToReference.svelte";
  import Inspector from "./components/Inspector.svelte";
  import InsertToolbar from "./components/InsertToolbar.svelte";
  import MarkerHelp from "./components/MarkerHelp.svelte";
  import MarkerStrip from "./components/MarkerStrip.svelte";
  import Navigator from "./components/Navigator.svelte";
  import Preview from "./components/preview/Preview.svelte";
  import PrintSettings from "./components/PrintSettings.svelte";
  import RecoveryPrompt from "./components/RecoveryPrompt.svelte";
  import SplitPane from "./components/SplitPane.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import UpdateBar from "./components/UpdateBar.svelte";
  import UpdatePrompt from "./components/UpdatePrompt.svelte";
  import { doc } from "./lib/document.svelte";
  import type { FileChanged } from "./lib/documentService";
  import { engine } from "./lib/engine.svelte";
  import { fonts } from "./lib/fonts.svelte";
  import { figures } from "./lib/figures.svelte";
  import {
    chapterVerse,
    insertionFor,
    insertionForMarker,
    markerClass,
  } from "./lib/insert";
  import { hasInvisibles } from "./lib/invisibles";
  import { layout } from "./lib/layout.svelte";
  import { markerAt } from "./lib/markerAt";
  import { markerTable } from "./lib/markerTable.svelte";
  import { convert, noteAt, notesIn, type NoteEntry } from "./lib/notes";
  import { chapterAt, chaptersOf, outlineOf, sectionAt, verseCountOf } from "./lib/outline";
  import { print } from "./lib/print.svelte";
  import { pwa } from "./lib/pwa.svelte";
  import { sidebarsIn } from "./lib/sidebars";
  import { updates } from "./lib/updates.svelte";
  import { ScrollSync, elementFor, scrollTo, topmostOffset, type Pane } from "./lib/scrollsync";
  import { SnapshotSchedule } from "./lib/snapshots";
  import type { LaunchQueueHost } from "./lib/launch";
  import { isDesktop } from "./lib/shell";
  import { theme, type Theme } from "./lib/theme.svelte";
  import { zoom } from "./lib/zoom.svelte";

  let editor: Editor | undefined = $state();
  let goto: GoToReference | undefined = $state();
  let find: FindBar | undefined = $state();
  let preview: Preview | undefined = $state();
  let printSettings: PrintSettings | undefined = $state();
  let recoveryPrompt: RecoveryPrompt | undefined = $state();
  let markerHelp: MarkerHelp | undefined = $state();
  let palette: CommandPalette | undefined = $state();

  /**
   * A change to the file made outside this window (FILE-FIDELITY 3, P4.4).
   *
   * Only ever set for the dirty and deleted cases. A clean document reloads
   * silently -- there is nothing to lose and nothing to ask.
   */
  let outside = $state<{ kind: "external" | "gone"; text: string | null } | null>(null);
  /** What the status bar says after a silent reload, briefly. */
  let reloaded = $state<string | null>(null);
  let error = $state<string | null>(null);
  /**
   * Show zero-width characters (UNICODE appendix).
   *
   * "Defaulting to on when the document's script uses them" -- a property of
   * the file, not of the application, so it is set per document rather than
   * remembered. A file with none of them gets a clean editor; a file with one
   * gets to see it, which is the case where it matters.
   */
  let showInvisibles = $state(false);

  /**
   * Asks the engine, moves the cursor, and reports what to say if it failed.
   *
   * The dialog stays open on failure with the reason, because the fix is
   * almost always one character — and because "GEN 1:1 is in a different file"
   * is not a failure the user can act on by retyping.
   */
  async function goToReference(text: string): Promise<string | null> {
    const result = await engine.resolve(text);
    // Tested for being a number rather than for not being null. This crosses
    // from another language, where "absent" has been both `null` and
    // `undefined` at different times, and a check that only catches one of
    // those hands `undefined` to the editor as a cursor position.
    if (typeof result.start !== "number" || typeof result.end !== "number") {
      return result.message ?? "That reference is not in this document.";
    }
    editor?.reveal(result.start, result.end);
    return null;
  }

  /**
   * Opens a link that came out of a document, outside the application.
   *
   * SECURITY 2: never in the webview -- a link opened there is a link running
   * in this application's own origin, which is the whole thing being defended
   * against. The URL has already been sanitized; this is the confirmation the
   * same section asks for, because following a link in a file someone else
   * sent is a request to that someone's server.
   */
  async function followLink(href: string): Promise<void> {
    if (!confirm(`Open this link outside Easy USFM?

${href}`)) return;

    if (isDesktop()) {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl(href);
      return;
    }
    // `noopener` so the opened page cannot reach back through `window.opener`.
    window.open(href, "_blank", "noopener,noreferrer");
  }

  /**
   * The two panes follow each other (P3.6).
   *
   * Scrolling one and not the other makes the split useless the moment a book
   * is longer than a screen: the translator scrolls the source, looks right,
   * and the preview is still on chapter one.
   *
   * Only the pane the user is actually scrolling drives the other; see
   * scrollsync.ts for why that is the guard rather than a timer. The pane being
   * moved never claims the wheel, so its own scroll events -- including the
   * corrections a virtualized editor emits for several frames after a jump --
   * are ignored, and the two cannot chase each other.
   *
   * (The pane sync itself is `sync`, below.)
   */

  /**
   * Recovery snapshots (FILE-FIDELITY 4, P4.1).
   *
   * The cadence lives here because this is the side that sees a keystroke.
   * Where snapshots go, what they contain, and how many survive is the shell's,
   * which is why this hands over a document and a caret and nothing else.
   */
  const snapshots = new SnapshotSchedule(() => void doc.snapshot(caret));

  /** Where the caret is, recorded so a recovery can put it back. */
  let caret = $state(0);
  /** Line and column, which only the editor can answer. */
  let position = $state<{ line: number; column: number } | null>(null);

  /**
   * Everything the navigator and the inspector show, derived from the parse.
   *
   * None of it is stored and none of it is pushed: the chapter grid, the
   * outline, the notes and the sidebars are all views of what the engine has
   * already said, so a chapter typed into existence appears in the grid
   * without anything being told to refresh.
   *
   * Scoped to the chapter the caret is in, which is what keeps the cost flat.
   * A book is two megabytes; a chapter is a few thousand characters, and every
   * scan below is over that.
   */
  const chapters = $derived(chaptersOf(engine.chunks, engine.diagnostics));
  const activeChapter = $derived(chapterAt(chapters, caret));
  const chapterNodes = $derived(engine.previews[activeChapter] ?? []);
  const sections = $derived(outlineOf(chapterNodes));
  const activeSection = $derived(sectionAt(sections, caret));

  /** How many verses the current chapter has, for the navigator's footer. */
  const verseCount = $derived(verseCountOf(chapterNodes));

  const notes = $derived(notesIn(chapterNodes));
  /**
   * Which note the inspector shows.
   *
   * The one the caret is in, and otherwise the last one it was in — so moving
   * out of a footnote to look at something does not blank the panel that was
   * being read. Reset when the chapter changes, which is a different set.
   */
  let noteChoice = $state(0);
  const noteHere = $derived(noteAt(notes, caret));
  const activeNote = $derived(
    noteHere >= 0 ? noteHere : Math.min(noteChoice, Math.max(0, notes.length - 1)),
  );

  /** The chapter's own text, and the sidebars in it. */
  const chapterSpan = $derived(engine.chunks[activeChapter]);
  const sidebars = $derived.by(() => {
    const span = chapterSpan;
    if (!span) return [];
    // `lineAt` rather than counting newlines: CodeMirror keeps a line index
    // and answers in logarithmic time, where counting them here would scan
    // every byte before the chapter on every keystroke.
    const firstLine = editor?.lineAt(span.start) ?? 1;
    return sidebarsIn(doc.text.slice(span.start, span.end), span.start, firstLine);
  });

  /** The marker the caret is inside, for the inspector's third tab. */
  const markerHere = $derived(markerAt(doc.text, caret));

  /** What the Study breadcrumb says. */
  const chapterLabel = $derived(
    chapters[activeChapter]?.number === null || chapters[activeChapter] === undefined
      ? null
      : String(chapters[activeChapter]?.number),
  );
  const sectionLabel = $derived(sections[activeSection]?.title ?? null);

  /**
   * Where the floating note card goes, in Study.
   *
   * Recomputed when the caret moves and when the editor scrolls, because both
   * change where the line is on screen. `null` while the note is off screen —
   * CodeMirror does not render what is not visible, so there are no
   * coordinates, and a card pinned to a guess sits over the wrong line.
   */
  let noteRect = $state<{ x: number; y: number; bottom: number } | null>(null);
  /** Escape closes it until the caret next lands in a note. */
  let noteDismissed = $state(false);

  function placeNote(): void {
    const note = notes[activeNote];
    const at = noteHere >= 0 && note?.start !== null && note?.start !== undefined ? note.start : null;
    noteRect = at === null ? null : (editor?.offsetRect(at) ?? null);
  }

  const sync = new ScrollSync();

  /**
   * Ctrl and the wheel zooms rather than scrolls.
   *
   * Attached rather than declared as `onwheel`, because it has to be
   * non-passive: `ctrlKey` with a wheel is also the browser's own page zoom,
   * and only a prevented event stops that happening as well. Svelte's
   * declarative handlers are passive for wheel, so `preventDefault` there is
   * ignored.
   *
   * On the window, not on each pane -- zoom is a property of the document being
   * read, not of whichever half the pointer happens to be over.
   */
  $effect(() => {
    const onWheel = (event: WheelEvent) => zoom.wheel(event);
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => window.removeEventListener("wheel", onWheel);
  });

  // The stored level, put where the stylesheet can see it. On mount rather than
  // at import, because it touches the document.
  $effect(() => {
    zoom.apply();
  });

  // Which pane the user is working in, which is what decides whether a scroll
  // event is an intent or an echo. After mount, because both panes have to
  // exist before they can be watched.
  $effect(() => {
    const panes: [Pane, HTMLElement | undefined][] = [
      ["editor", editor?.scroller()],
      ["preview", preview?.container()],
    ];

    const stops: (() => void)[] = [];
    for (const [pane, element] of panes) {
      if (element) stops.push(sync.watch(pane, element));
    }

    return () => {
      for (const stop of stops) stop();
    };
  });

  function editorScrolled(): void {
    if (!sync.accepts("editor")) return;

    const offset = editor?.topOffset();
    const container = preview?.container();
    if (offset === null || offset === undefined || !container) return;

    const target = elementFor(container, offset);
    if (!target) return;

    scrollTo(container, target);
  }

  function previewScrolled(): void {
    if (!sync.accepts("preview")) return;

    const container = preview?.container();
    if (!container) return;

    const offset = topmostOffset(container);
    if (offset === null) return;

    editor?.scrollToOffset(offset);
  }

  /**
   * A save settles the schedule.
   *
   * Cancelling rather than flushing, because the file on disk now holds the
   * work -- a snapshot taken here would record what was just saved, and
   * FILE-FIDELITY 4 has snapshots *cleared* at this moment rather than written.
   */
  async function saved(action: () => Promise<boolean>): Promise<void> {
    await run(async () => {
      if (await action()) snapshots.settled();
    });
  }

  /**
   * Asks what is waiting for the file just opened, and acts on it
   * (FILE-FIDELITY 4, P4.2).
   *
   * Three outcomes. Another live instance holds it, so the editor refuses
   * edits until the user takes over. A session did not finish and left work
   * that differs from disk, so the offer is made -- and it is an offer, never
   * applied on its own. Otherwise the lock is taken and the file is just open.
   */
  async function claim(): Promise<void> {
    doc.readOnly = false;
    doc.heldBy = null;

    const path = doc.path;
    // A document never saved has no file for anyone to hold and no snapshot
    // filed under a path.
    if (!path) return;

    const waiting = await doc.examine(path);
    if (!waiting) return;

    if (waiting.held.state === "foreign") {
      doc.readOnly = true;
      doc.heldBy = waiting.held.owner;
      // Deliberately no lock taken and no recovery offered. Both belong to the
      // instance that has it.
      return;
    }

    await doc.takeOver(path);
    if (waiting.recovery) recoveryPrompt?.ask(waiting.recovery);
    await doc.watch(fileChanged);
  }

  /**
   * The file changed underneath us (FILE-FIDELITY 3).
   *
   * Clean means reload silently, preserving position **by verse reference**
   * rather than by offset -- an external rewrite makes offsets meaningless, and
   * landing the caret at byte 4,000 of a file somebody reformatted puts it
   * somewhere arbitrary. Dirty means the non-modal bar, never an automatic
   * overwrite. Deleted keeps the buffer and says Save will recreate it.
   */
  async function fileChanged(change: FileChanged): Promise<void> {
    if (change.kind === "gone") {
      // Marked dirty: the buffer is now the only copy, and closing without
      // saving would lose it.
      doc.dirty = true;
      outside = { kind: "gone", text: null };
      return;
    }
    if (doc.dirty) {
      outside = { kind: "external", text: change.text };
      return;
    }
    if (change.text !== null) await reloadFrom(change.text);
  }

  /**
   * Replaces the buffer, putting the caret back at the same verse.
   *
   * The reference is asked for *before* the text changes and resolved
   * afterwards, because it is the one coordinate that survives a rewrite. A
   * reference that no longer exists in the new text simply leaves the caret at
   * the top, which is honest -- the verse it named is gone.
   */
  async function reloadFrom(text: string): Promise<void> {
    const wasAt = engine.reference;

    doc.reload(text);
    editor?.load(doc.text);
    engine.open(doc.text);
    void fonts.inspect(doc.text);
    showInvisibles = hasInvisibles(doc.text);
    snapshots.settled();
    outside = null;

    if (wasAt) {
      const found = await engine.resolve(wasAt);
      if (typeof found.start === "number" && typeof found.end === "number") {
        editor?.reveal(found.start, found.end, false);
      }
    }

    reloaded = "Reloaded from disk";
    setTimeout(() => (reloaded = null), 4000);
  }

  /**
   * Detaches the buffer from the file, so it can be saved elsewhere.
   *
   * The text is kept and the path is dropped, which turns Save into Save As.
   * Nothing is copied on disk -- the user chooses where it goes.
   */
  async function openCopy(): Promise<void> {
    const held = doc.path;
    doc.detach();
    doc.readOnly = false;
    doc.heldBy = null;
    if (held) void doc.releaseLock(held);
  }

  /**
   * Claims a file another live instance holds.
   *
   * Confirmed, because §4 asks for the warning: the other window still has the
   * document and may save over whatever is written here.
   */
  async function takeOver(): Promise<void> {
    const path = doc.path;
    if (!path) return;
    const warning = [
      "Another Easy USFM window has this file open.",
      "Taking over does not close it, and that window may still save over your changes.",
      "Take over anyway?",
    ].join("\n\n");
    if (!confirm(warning)) return;
    await doc.takeOver(path);
  }

  /**
   * Shows what is on disk beside what is in the editor.
   *
   * FILE-FIDELITY 3 offers **Compare** and this application is not a diff tool,
   * so the honest version is to say how they differ and let the user decide,
   * rather than to grow a three-way merge nobody asked for. PTXprint and Git
   * are where a real comparison belongs.
   */
  async function compareWithDisk(): Promise<void> {
    const disk = outside?.text;
    if (disk === null || disk === undefined) return;

    const mine = doc.text.split("\n");
    const theirs = disk.split("\n");
    let differing = 0;
    for (let line = 0; line < Math.max(mine.length, theirs.length); line += 1) {
      if (mine[line] !== theirs[line]) differing += 1;
    }

    alert(
      [
        `The copy on disk differs from yours on ${differing} ${differing === 1 ? "line" : "lines"}.`,
        `Yours: ${mine.length} lines. On disk: ${theirs.length} lines.`,
        "Nothing has been changed. Keep my version leaves the file alone; " +
          "Reload discards your changes.",
      ].join("\n\n"),
    );
  }

  const counts = $derived(engine.counts);

  /**
   * Images go back off whenever the document changes (SECURITY 3).
   *
   * The opt-in is per document, so trusting one file's images is not a decision
   * that carries to the next one. Keyed on the generation rather than on the
   * identifier or the path -- see `DocumentState.generation` for the two ways
   * those get it wrong, one of them silently and only in a browser.
   */
  $effect(() => {
    figures.reset(doc.generation);
  });

  /**
   * Print settings are per document too, but keyed by path rather than by
   * generation: they are a property of the book, so reopening a file should
   * find the paper size it was last printed on.
   */
  $effect(() => {
    print.load(doc.path);
  });

  /**
   * The editor is told about diagnostics as they arrive.
   *
   * Pushed here rather than passed as a prop because they are not the editor's
   * state — they are a parse result that has to be *mapped onto* whatever the
   * document has become since, which is something only the editor can do.
   */
  $effect(() => {
    editor?.applyDiagnostics(engine.diagnostics);
  });

  onMount(async () => {
    engine.ontokens = (from, to, tokens) => editor?.applyTokens(from, to, tokens);
    engine.start();
    await run(() => doc.createNew());
    // The editor was constructed before the document existed, so it is given
    // the text explicitly rather than relying on the prop it was mounted with.
    editor?.load(doc.text);
    engine.open(doc.text);
    void fonts.inspect(doc.text);
    showInvisibles = hasInvisibles(doc.text);

    if (!isDesktop()) {
      guardTheTab();
      // The service worker is what makes the offline claim true (P5.1). It
      // installs in the background and never takes over on its own.
      void pwa.register();
      await openWhatTheSystemHandedUs();
    }

    if (isDesktop()) {
      await guardTheWindow();
      await listenToMenu();
      // The shell starts with an empty Open Recent; the list lives here.
      await doc.pushRecentToMenu();

      // Whether this build can check for updates at all, and then the first-run
      // question (PRODUCT 11). Asked after the document is open and only while
      // nothing is unsaved -- a prompt over somebody's typing is dismissed by
      // the next keystroke, which is consent nobody gave.
      const { invoke } = await import("@tauri-apps/api/core");
      await updates.inspect(invoke);
      updates.askIfNeeded(doc.dirty);
    }
  });

  /**
   * Every command, by the identifier the native menu emits.
   *
   * Four things ask for a command now — the native menu, the browser build's
   * menu bar, the toolbar and the command palette — and all four send an id
   * here. That is what stops the palette from becoming a second
   * implementation of Save: there is one switch, and a route that cannot name
   * an id in it does nothing rather than doing something slightly different.
   */
  async function command(id: string): Promise<void> {
    if (id.startsWith("recent:")) {
      const path = id.slice("recent:".length);
      if (path === "clear") doc.clearRecent();
      else if (path !== "none") await load(() => doc.open(path));
      return;
    }
    if (id.startsWith("theme:")) {
      theme.set(id.slice("theme:".length) as Theme);
      return;
    }

    // Every insert command, by the id the toolbar uses. Handled before the
    // switch so adding one to `COMMANDS` adds it to the menu path too.
    if (id.startsWith("insert-")) {
      insert(id);
      return;
    }

    switch (id) {
      case "new":
        await load(() => doc.createNew());
        break;
      case "open":
        await load(() => doc.open());
        break;
      case "save":
        await saved(() => doc.save());
        break;
      case "save-as":
        await saved(() => doc.saveAs());
        break;
      case "focus-preview":
        focusPane("Preview");
        break;

      case "cycle-pane":
        cyclePanes(true);
        break;

      // Chooses, rather than cycling. PRODUCT 6.4 gives Ctrl+1 and Ctrl+2 as
      // "focus editor / preview" and F6 separately as "cycle pane focus";
      // this item was cycling, so the two shortcuts did the same thing and
      // neither did what the table says.
      case "focus-editor":
        editor?.focus();
        break;
      case "print":
        printSettings?.open();
        break;
      case "toggle-images":
        figures.toggle(!figures.shown);
        break;

      case "toggle-invisibles":
        showInvisibles = !showInvisibles;
        break;
      case "toggle-diagnostics":
        layout.toggleDiagnostics();
        break;
      case "next-diagnostic":
        editor?.step(true);
        break;
      case "previous-diagnostic":
        editor?.step(false);
        break;
      case "zoom-in":
        zoom.in();
        break;

      case "zoom-out":
        zoom.out();
        break;

      case "zoom-reset":
        zoom.reset();
        break;

      case "marker-reference":
        void markerHelp?.open();
        break;

      case "go-to-reference":
        goto?.open();
        break;
      case "find":
        find?.show(false);
        break;
      case "replace":
        find?.show(true);
        break;
      case "find-next":
        find?.step(true);
        break;
      case "find-previous":
        find?.step(false);
        break;

      // The redesign's own commands. Layout is a working habit rather than a
      // property of the document, so all of it is remembered (`layout`).
      case "palette":
        palette?.open();
        break;
      case "shell-workbench":
        layout.setShell("workbench");
        break;
      case "shell-study":
        layout.setShell("study");
        break;
      case "panes-editor":
        layout.setPanes("editor");
        break;
      case "panes-split":
        layout.setPanes("split");
        break;
      case "panes-preview":
        layout.setPanes("preview");
        break;
      case "toggle-sync":
        layout.toggleSync();
        break;
      case "toggle-navigator":
        layout.toggleNavigator();
        break;
    }
  }

  /**
   * The native menu bar is another way to ask for the same commands.
   *
   * One listener with the item's id as the payload, so adding an item to the
   * menu does not mean adding a listener here — and so the menu can never
   * drift into being a second implementation of anything.
   */
  async function listenToMenu(): Promise<void> {
    const { listen } = await import("@tauri-apps/api/event");
    await listen<string>("menu", (event) => void command(event.payload));
  }

  // Separate from onMount, which cannot return a cleanup when it is async.
  onDestroy(() => {
    engine.stop();
    // The last chance to record work that is still only in memory. A no-op
    // when nothing is outstanding, so a clean close leaves nothing behind for
    // the next launch to offer back.
    snapshots.flush();
  });

  /**
   * The window title.
   *
   * `document.title` is the webview's, which on Windows and Linux is not what
   * the title bar shows -- the native frame carries the title from the Tauri
   * configuration, so the file name and the unsaved marker never reached it.
   * Both are set, because the webview's is what a browser tab shows.
   */
  $effect(() => {
    document.title = doc.title;
    if (isDesktop()) void setWindowTitle(doc.title);
  });

  async function setWindowTitle(title: string): Promise<void> {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTitle(title);
  }

  /**
   * Line height follows the document's scripts (UNICODE 7).
   *
   * On the root rather than the editor, so the preview and the diagnostics
   * panel -- which show the same Scripture -- are set the same way.
   */
  $effect(() => {
    document.documentElement.style.setProperty("--line-height", String(fonts.lineHeight));
  });

  /**
   * F6 moves to the next pane, Shift+F6 to the previous.
   *
   * PRODUCT 6.4 calls this "cycle pane focus" and 10 says "F6 cycles"; it
   * focused the editor and nothing else, which is not a cycle -- pressing it
   * twice did what pressing it once did, and the diagnostics panel was
   * reachable only by tabbing through the document.
   *
   * The panes are found in the DOM rather than listed here, so a pane that
   * exists is a pane F6 reaches. The preview arrives with M3 and will join the
   * cycle by being rendered, not by being added to a list somebody has to
   * remember.
   */
  function cyclePanes(forward: boolean): void {
    const panes = [...document.querySelectorAll<HTMLElement>("[data-pane]")].filter(
      // A collapsed panel is not somewhere focus can usefully go.
      (pane) => pane.offsetParent !== null,
    );
    if (panes.length === 0) return;

    const at = panes.findIndex((pane) => pane.contains(document.activeElement));
    const next = panes[(at + (forward ? 1 : -1) + panes.length) % panes.length];

    // The pane itself is a region, not a control. Focus goes to the thing
    // inside it that takes keys -- the editor's content, the diagnostics
    // list -- and falls back to the region only when there is nothing.
    const target = next?.querySelector<HTMLElement>("[data-pane-focus]") ?? next;
    target?.focus();
  }

  /**
   * Runs an insert command, from the toolbar or the menu.
   *
   * The numbers are suggested rather than imposed: `\c` and `\v` are given the
   * next one where it can be worked out, with the caret after it so a different
   * one is a keystroke away. Refusing while the document is read-only, because
   * the editor would refuse the transaction anyway and a button that silently
   * does nothing is worse than one that is visibly unavailable.
   */
  function insert(id: string): void {
    if (doc.readOnly) return;

    const at = editor?.selection();
    if (!at) return;

    const insertion = insertionFor(id, where(at));
    if (!insertion) return;

    editor?.applyInsertion(at.from, at.to, insertion);
  }

  /** What an insertion needs to know about the document and the caret. */
  function where(at: { text: string; from: number; to: number }) {
    return {
      text: at.text,
      from: at.from,
      to: at.to,
      nextChapter: nextChapter(),
      nextVerse: nextVerse(),
      // For a note's reference marker, which is the one field that can be
      // filled in from where the caret already is.
      reference: engine.reference ?? undefined,
    };
  }

  /**
   * Inserts a marker chosen by name, from the marker strip or the palette.
   *
   * Through the command where one exists, so `\c` picked out of a list writes
   * exactly what the Chapter button writes. Otherwise the marker table says
   * what shape it needs — wrapping, a line of its own, self-closing — which is
   * the part that decides whether the file still parses.
   */
  async function insertMarker(marker: string): Promise<void> {
    if (doc.readOnly) return;

    const at = editor?.selection();
    if (!at) return;

    // Awaited rather than guessed at. The table is fetched once per session
    // and is usually already here, because picking a marker by name means
    // something has already listed them.
    await markerTable.load();
    const kind = markerClass(markerTable.find(marker)?.class);

    editor?.applyInsertion(at.from, at.to, insertionForMarker(marker, kind, where(at)));
  }

  /**
   * One past the highest chapter the document has.
   *
   * From the parse rather than by scanning the text: the engine already knows
   * where the chapters are, and a second reader of `\c` here would be a second
   * thing to keep in step with the parser.
   */
  function nextChapter(): number | undefined {
    const numbers = engine.chunks
      .map((chunk) => chunk.number)
      .filter((number): number is number => number !== null);

    return numbers.length === 0 ? 1 : Math.max(...numbers) + 1;
  }

  /**
   * One past the verse the caret is in.
   *
   * From the status bar's own reference, which the engine keeps current as the
   * caret moves. `undefined` where there is no verse yet -- in a chapter that
   * has none, the useful suggestion is 1, and outside a chapter there is
   * nothing to suggest at all.
   */
  function nextVerse(): number | undefined {
    const reference = engine.reference;
    if (!reference) return undefined;

    const verse = /:(\d+)\s*$/.exec(reference);
    if (!verse) return 1;

    const parsed = Number(verse[1]);
    return Number.isFinite(parsed) ? parsed + 1 : undefined;
  }

  /**
   * Moves the caret to an offset and shows it.
   *
   * The navigator, the outline, the sidebar list and the note card all do this
   * and all mean the same thing by it: put the cursor there and let me see it.
   */
  function goToOffset(offset: number): void {
    editor?.reveal(offset, offset);
  }

  /**
   * Replaces a span of the document, from a panel rather than from typing.
   *
   * Through the editor's own transaction, so it is one undo step and the
   * buffer stays the authority (ADR-003). A panel that wrote to `doc.text`
   * directly would be a second writer to the document, and the engine's mirror
   * would never hear about it.
   */
  function applySpan(from: number, to: number, text: string): void {
    if (doc.readOnly) return;
    editor?.replaceRange(from, to, text);
  }

  /** Turns a footnote into a cross-reference, or back (P6 inspector). */
  function convertNote(note: NoteEntry): void {
    if (doc.readOnly || note.start === null || note.end === null) return;

    const source = doc.text.slice(note.start, note.end);
    const converted = convert(source);
    // `null` means there was nothing to change, so the document is left alone
    // rather than dispatching a transaction that does nothing and costs an
    // undo step.
    if (converted === null) return;

    editor?.replaceRange(note.start, note.end, converted);
  }

  /**
   * Puts focus in one named pane.
   *
   * Through the same `[data-pane]` regions F6 walks, rather than by reaching
   * for a component's element: the scrolling `.preview` div is not focusable,
   * so focusing it did nothing at all and Ctrl+2 was a shortcut that appeared
   * to work and did not. Whatever inside the pane takes keys is the target,
   * falling back to the region -- which is the rule F6 already follows.
   */
  function focusPane(label: string): void {
    const pane = [...document.querySelectorAll<HTMLElement>("[data-pane]")].find(
      (candidate) => candidate.getAttribute("aria-label") === label,
    );
    if (!pane) return;

    (pane.querySelector<HTMLElement>("[data-pane-focus]") ?? pane).focus();
  }

  /** Anything that touches a file can fail; none of it should be silent. */
  async function run(action: () => Promise<unknown>): Promise<void> {
    try {
      error = null;
      await action();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }

  async function load(action: () => Promise<unknown>): Promise<void> {
    if (!(await doc.confirmDiscard())) return;

    // Whatever was pending belongs to the document being replaced. Flushed
    // rather than dropped: the user has just chosen to discard their changes
    // in the interface, and that is not the same as choosing to lose them --
    // the snapshot is what makes the choice reversible.
    snapshots.flush();
    // The document being replaced is no longer ours to hold.
    const leaving = doc.path;
    await run(action);
    // The new document starts with nothing outstanding.
    snapshots.settled();
    if (leaving && leaving !== doc.path) void doc.releaseLock(leaving);
    // Stop watching whatever we were on. `claim` starts the new watch, and
    // leaving this would report changes to a file nobody is looking at.
    outside = null;
    await doc.unwatch();
    await claim();
    editor?.load(doc.text);
    // A new document is not an edit to the old one; the engine gets the whole
    // text rather than a delta it could not make sense of.
    engine.open(doc.text);
    // Asked per document, not per keystroke: which scripts a file uses is a
    // property of the file, and typing does not introduce one.
    void fonts.inspect(doc.text);
    showInvisibles = hasInvisibles(doc.text);
  }

  /**
   * Closing the window must not discard unsaved work.
   *
   * PRODUCT §3 lists the unsaved-change warning as part of the lifecycle, and
   * the shell has to be asked not to close rather than told afterwards.
   */
  async function guardTheWindow(): Promise<void> {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const window = getCurrentWindow();

    await window.onCloseRequested(async (event) => {
      if (!(await doc.confirmDiscard())) {
        event.preventDefault();
        return;
      }

      // A clean close, so this file is no longer ours (FILE-FIDELITY 4). Left
      // behind, the lock reads as a crash on the next launch and offers a
      // recovery nobody needs. Awaited: the window is about to be destroyed,
      // and a promise in flight when that happens never lands.
      const held = doc.path;
      if (held) await doc.releaseLock(held);
      await doc.unwatch();
      // The user chose to discard, so there is nothing outstanding worth
      // keeping -- and a snapshot written here would be offered back next time.
      if (!doc.dirty) await doc.clearSnapshots();
    });
  }

  /**
   * A file the operating system handed to the installed application (P5.2).
   *
   * `launchQueue` is how a PWA registered as a `file_handlers` target receives
   * the file that was double-clicked. It has to be consumed early: the browser
   * holds the launch until a consumer is set, and setting one after the first
   * paint means the file arrives at a document already open.
   */
  async function openWhatTheSystemHandedUs(): Promise<void> {
    if (!("launchQueue" in window)) return;

    (window as unknown as LaunchQueueHost).launchQueue.setConsumer((launch) => {
      const handle = launch.files?.[0];
      if (handle) void load(() => doc.adopt(handle));
    });
  }

  /**
   * The browser's two teardown hooks (FILE-FIDELITY 4, P4.6).
   *
   * `beforeunload` warns on unsaved work, and is the only thing a browser
   * offers for that -- the text of the prompt is the browser's own and cannot
   * be set, which is why the snapshot matters more here than on the desktop.
   *
   * The snapshot itself flushes on `visibilitychange -> hidden`, "the only
   * reliably-fired teardown event". `beforeunload` and `unload` are not fired
   * when a tab is discarded under memory pressure or when a phone kills a
   * background page, and those are exactly the cases a recovery snapshot is
   * for.
   */
  function guardTheTab(): void {
    window.addEventListener("beforeunload", (event) => {
      if (!doc.dirty) return;
      event.preventDefault();
      // Assigning `returnValue` is the older spelling and still what some
      // browsers require to show the prompt at all.
      event.returnValue = "";
    });

    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") snapshots.flush();
    });
  }

  /**
   * Shortcuts for the browser build only.
   *
   * On the desktop these are the menu's accelerators, declared beside the
   * items so the menu shows the key that actually works. Handling them here as
   * well would fire every command twice.
   */
  function onKeyDown(event: KeyboardEvent): void {
    if (isDesktop()) return;

    // Ctrl on Windows and Linux, Command on macOS (PRODUCT §7).
    const accel = event.ctrlKey || event.metaKey;

    if (event.key === "F6") {
      event.preventDefault();
      cyclePanes(!event.shiftKey);
      return;
    }
    if (event.key === "F8") {
      event.preventDefault();
      editor?.step(!event.shiftKey);
      return;
    }
    if (event.key === "F3") {
      event.preventDefault();
      find?.step(!event.shiftKey);
      return;
    }
    if (!accel) return;

    // Ctrl+Shift+M and Ctrl+Shift+8. Checked on `code` rather than `key`,
    // because with Shift held the key a layout reports is not reliably the
    // one printed on the cap.
    if (event.shiftKey && event.code === "KeyM") {
      event.preventDefault();
      layout.toggleDiagnostics();
      return;
    }
    if (event.shiftKey && event.code === "Digit8") {
      event.preventDefault();
      showInvisibles = !showInvisibles;
      return;
    }

    // Ctrl+G on Windows and Linux; ⌘L on macOS, where ⌘G is Find Next by
    // universal convention and is reserved for it (PRODUCT §6.4).
    const wantsGoTo = event.metaKey ? event.key.toLowerCase() === "l" : event.key.toLowerCase() === "g";
    if (wantsGoTo) {
      event.preventDefault();
      goto?.open();
      return;
    }

    switch (event.key.toLowerCase()) {
      // The palette. Ctrl+K everywhere, because it is the shortcut every
      // editor written in the last decade uses for exactly this.
      case "k":
        event.preventDefault();
        palette?.open();
        break;

      case "f":
        event.preventDefault();
        find?.show(false);
        break;
      case "h":
        event.preventDefault();
        find?.show(true);
        break;
      case "n":
        event.preventDefault();
        void load(() => doc.createNew());
        break;
      case "o":
        event.preventDefault();
        void load(() => doc.open());
        break;
      case "s":
        event.preventDefault();
        void saved(() => (event.shiftKey ? doc.saveAs() : doc.save()));
        break;
      // Zoom. Both spellings of the plus key: `+` is shifted on most layouts
      // and `=` is the same physical key, so accepting only one makes the
      // shortcut need a modifier nobody expects.
      case "+":
      case "=":
        event.preventDefault();
        zoom.in();
        break;
      case "-":
        event.preventDefault();
        zoom.out();
        break;
      case "0":
        event.preventDefault();
        zoom.reset();
        break;

      case "p":
        // The panel first, not the printer. Every setting in it changes what
        // comes out, and the browser's own dialog cannot ask about any of them
        // (PRODUCT 8). Print is the button at the end of it.
        event.preventDefault();
        printSettings?.open();
        break;
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<!--
  The window, in one of two shapes (`lib/layout.svelte.ts`).

  Workbench is four ruled columns with the apparatus docked; Study narrows the
  navigator to a rail, widens the reading column and floats the apparatus
  beside what it describes. The panes themselves are the same components in
  both — what differs is how much room each is given.
-->
<div class="app" data-shell={layout.shell}>
  <AppBar
    onrun={(id) => void command(id)}
    onpalette={() => palette?.open()}
    chapter={chapterLabel}
    section={sectionLabel}
    reference={engine.reference}
  />

  {#if layout.shell === "workbench"}
    <DocumentBar
      onrun={(id) => void command(id)}
      usfm={engine.usfm.effective}
      errors={counts.error}
      warnings={counts.warning}
      ready={engine.ready}
    />
  {/if}

  <MarkerHelp bind:this={markerHelp} />

  <PrintSettings bind:this={printSettings} saved={doc.path !== null} />

  <RecoveryPrompt
    bind:this={recoveryPrompt}
    onrestore={(recovery) => {
      doc.restore(recovery.text);
      editor?.load(doc.text);
      engine.open(doc.text);
      void fonts.inspect(doc.text);
      showInvisibles = hasInvisibles(doc.text);
      // Where the caret was when the snapshot was taken, so the reader lands
      // where they left off rather than at the top of a file they were in the
      // middle of.
      editor?.reveal(recovery.cursor, recovery.cursor);
      // Restored work is unsaved by definition, so the schedule starts again.
      snapshots.changed();
    }}
    ondiscard={() => void doc.clearSnapshots()}
  />

  <CommandPalette
    bind:this={palette}
    diagnostics={engine.diagnostics}
    onrun={(id) => void command(id)}
    onmarker={(marker) => void insertMarker(marker)}
    onreference={goToReference}
    ondiagnostic={(index) => editor?.goTo(index, true)}
    onclose={() => editor?.focus()}
  />

  <UpdateBar />
  <UpdatePrompt />

  {#if outside}
    <ExternalChange
      kind={outside.kind}
      onreload={() => {
        if (outside?.text !== null && outside?.text !== undefined) void reloadFrom(outside.text);
      }}
      onkeep={() => (outside = null)}
      oncompare={() => void compareWithDisk()}
    />
  {/if}

  {#if doc.readOnly}
    <!--
      Non-modal, because the file is perfectly readable and the user may have
      opened it only to look. FILE-FIDELITY 4's two choices are here rather
      than in a dialog that has to be dismissed before anything can be seen.
    -->
    <p class="held" role="status">
      Another Easy USFM window has this file open{doc.heldBy
        ? ` (process ${doc.heldBy.pid})`
        : ""}. It is read-only here.
      <button type="button" onclick={() => void openCopy()}>Open a copy</button>
      <button type="button" onclick={() => void takeOver()}>Take over</button>
    </p>
  {/if}

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <FontNotice notices={fonts.notices} ondismiss={() => fonts.dismiss()} />

  <main class="body">
    {#if layout.navigator || layout.shell === "study"}
      <Navigator
        {chapters}
        {sections}
        {activeChapter}
        {activeSection}
        verses={verseCount}
        ongo={goToOffset}
      />
    {:else}
      <!-- The folded navigator leaves a way back. Anything smaller than this
           is a control nobody finds again. -->
      <button
        type="button"
        class="unfold"
        title="Show the navigator"
        onclick={() => layout.toggleNavigator()}
      >
        ›
        <span class="visually-hidden">Show the navigator</span>
      </button>
    {/if}

    <div class="panes">
      {#if layout.panes === "split"}
        <SplitPane id="main" startLabel="USFM source" endLabel="Preview">
          {#snippet start()}
            {@render editorPane()}
          {/snippet}

          {#snippet end()}
            {@render previewPane()}
          {/snippet}
        </SplitPane>
      {:else if layout.panes === "editor"}
        {@render editorPane()}
      {:else}
        {@render previewPane()}
      {/if}
    </div>

    {#if layout.shell === "workbench"}
      <Inspector
        {sidebars}
        {notes}
        note={activeNote}
        marker={markerHere}
        chapter={chapterLabel}
        newAt={chapterVerse(engine.reference ?? undefined)}
        readOnly={doc.readOnly}
        onapply={applySpan}
        ongo={goToOffset}
        onnewsidebar={() => insert("insert-sidebar")}
        onnote={(index) => (noteChoice = index)}
        onconvert={convertNote}
        onreference={() => void markerHelp?.open()}
      />
    {/if}
  </main>

  {#if layout.shell === "study"}
    <MarkerStrip
      onmarker={(marker) => void insertMarker(marker)}
      ondiagnostics={() => layout.toggleDiagnostics()}
      errors={counts.error}
      warnings={counts.warning}
      disabled={doc.readOnly}
    />
  {/if}

  <FindBar
    bind:this={find}
    onsearch={(query, exact) => engine.find(query, exact)}
    onreveal={(match, focus) => editor?.reveal(match.start, match.end, focus)}
    onreplace={(match, text) => editor?.replaceRange(match.start, match.end, text)}
    onreplaceall={(all, text) =>
      editor?.replaceAll(
        all.map((match) => ({ from: match.start, to: match.end })),
        text,
      )}
    onclose={() => editor?.focus()}
  />

  <GoToReference bind:this={goto} onsubmit={goToReference} onclose={() => editor?.focus()} />

  <DiagnosticsPanel
    diagnostics={engine.diagnostics}
    open={layout.diagnostics}
    ontoggle={() => layout.toggleDiagnostics()}
    onselect={(index, focus) => editor?.goTo(index, focus)}
    onescape={() => editor?.focus()}
  />

  <StatusBar {position} notice={reloaded} errors={counts.error} warnings={counts.warning} />
</div>

<!--
  The editor and the preview, written once and placed by whichever arrangement
  is showing. Snippets rather than two copies of the markup: a split pane and a
  single pane differ in where the component goes, not in what it is, and two
  copies would eventually disagree about one of a dozen props.
-->
{#snippet editorPane()}
  <div class="pane">
    <!--
      Workbench only. Study has the marker strip along the bottom of the
      window, and two rows of the same markers a few hundred pixels apart is
      one row too many -- the strip is wider, grouped, and says what each
      marker is, which is the version worth keeping when there is room for it.
    -->
    {#if layout.shell === "workbench"}
      <InsertToolbar
        oninsert={insert}
        disabled={doc.readOnly}
        onexpand={() => layout.setPanes(layout.panes === "editor" ? "split" : "editor")}
        oninvisibles={() => (showInvisibles = !showInvisibles)}
        expanded={layout.panes === "editor"}
        invisibles={showInvisibles}
      />
    {/if}

    <div class="pane-body">
      <Editor
        bind:this={editor}
        value={doc.text}
        onchange={(text, changes) => {
          doc.edited(text, changes);
          // Typing can introduce a script the document did not have, which
          // changes both the leading and whether there is a font for it.
          fonts.schedule(text);
          snapshots.changed();
          engine.edit(
            changes.map((change) => ({
              from: change.fromA,
              to: change.toA,
              insert: change.inserted,
            })),
            text,
          );
        }}
        oncompositionstart={() => engine.startComposition()}
        oncompositionend={(text) => engine.endComposition(text)}
        ontokenrange={(from, to) => engine.requestTokens(from, to)}
        oncursor={(at) => {
          caret = at;
          position = editor?.position() ?? null;
          // A caret that has landed in a note is a note worth showing again.
          noteDismissed = false;
          placeNote();
          engine.locate(at);
        }}
        oncomplete={(at) => engine.completions(at)}
        {showInvisibles}
        readOnly={doc.readOnly}
        onscroll={() => {
          editorScrolled();
          // The card is pinned to a line, and the line has moved.
          placeNote();
        }}
      />

      <!--
        Study has no inspector column, so the apparatus comes to the text.
        Inside the pane rather than over the window, so it is positioned
        against the editor's own box and scrolls out of the way with it.
      -->
      {#if layout.shell === "study" && !noteDismissed}
        <FloatingNote
          {notes}
          selected={activeNote}
          at={noteRect}
          readOnly={doc.readOnly}
          onselect={(index) => (noteChoice = index)}
          ongo={goToOffset}
          onconvert={convertNote}
          onclose={() => (noteDismissed = true)}
        />
      {/if}
    </div>
  </div>
{/snippet}

{#snippet previewPane()}
  <div class="pane">
    <div class="pane-head">
      <span class="pane-name">Preview</span>
      <span class="pane-note">
        {#if engine.reference}{engine.reference}{:else}Reading pane{/if}
        {#if layout.sync}· synced{/if}
      </span>
      <span class="zoomers">
        <button type="button" title="Smaller" aria-label="Zoom out" onclick={() => zoom.out()}>
          A−
        </button>
        <button type="button" title="Larger" aria-label="Zoom in" onclick={() => zoom.in()}>
          A+
        </button>
      </span>
    </div>

    <div class="pane-body">
      <Preview
        bind:this={preview}
        onscroll={previewScrolled}
        chunks={engine.chunks}
        previews={engine.previews}
        onselect={(start, end) => editor?.reveal(start, end, false)}
        onfollow={(href) => void followLink(href)}
        onreference={(reference) => void goToReference(reference)}
        onfigure={(path) => void figures.request(doc, path)}
        onneed={(chunk) => engine.requestPreview(chunk)}
      />
    </div>
  </div>
{/snippet}

<style>
  .app {
    display: flex;
    flex-direction: column;
    block-size: 100%;
    background: var(--bg);
  }

  /*
   * The body, in the two arrangements the redesign draws.
   *
   * Fixed columns for the apparatus and fractional ones for the document,
   * because the navigator and the inspector hold labels of a known size and
   * the panes hold text that should take whatever is left.
   */
  .body {
    flex: 1 1 auto;
    min-block-size: 0;
    display: grid;
    grid-template-columns: 13rem minmax(0, 1fr) 18.25rem;
  }

  .app[data-shell="study"] .body {
    grid-template-columns: 3.25rem minmax(0, 1fr);
  }

  /* Folded away, the navigator leaves a hairline column with the way back. */
  .body:has(> .unfold) {
    grid-template-columns: 1.25rem minmax(0, 1fr) 18.25rem;
  }

  .app[data-shell="study"] .body:has(> .unfold) {
    grid-template-columns: 1.25rem minmax(0, 1fr);
  }

  .unfold {
    border: none;
    border-inline-end: 1px solid var(--line);
    background: var(--bg2);
    color: var(--fg3);
    font: inherit;
    cursor: pointer;
    padding: 0;
  }

  .unfold:hover {
    color: var(--acc-text);
  }

  .panes {
    min-inline-size: 0;
    min-block-size: 0;
    border-inline-end: 1px solid var(--line);
  }

  .app[data-shell="study"] .panes {
    border-inline-end: none;
  }

  .pane {
    display: flex;
    flex-direction: column;
    block-size: 100%;
    min-block-size: 0;
  }

  /* Positioned, so the floating note card measures against this box. */
  .pane-body {
    flex: 1 1 auto;
    min-block-size: 0;
    position: relative;
  }

  .pane-head {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    block-size: 2.375rem;
    padding-inline: 0.875rem;
    border-block-end: 1px solid var(--line2);
    font-size: 0.75rem;
    color: var(--fg2);
    flex: 0 0 auto;
  }

  .pane-name {
    font-weight: 500;
    color: var(--fg);
  }

  .pane-note {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .zoomers {
    margin-inline-start: auto;
    display: flex;
    gap: 0.1875rem;
    flex: 0 0 auto;
  }

  .zoomers button {
    padding-block: 0.125rem;
    padding-inline: 0.4375rem;
    border: none;
    border-radius: var(--radius-sm);
    background: var(--bg3);
    color: var(--fg2);
    font: inherit;
    font-size: 0.71875rem;
    cursor: pointer;
  }

  .zoomers button:hover {
    background: var(--hl);
    color: var(--fg);
  }

  /* Another window has this file. A notice rather than a dialog: the file is
     perfectly readable and someone may have opened it only to look. */
  .held {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
    margin: 0;
    padding-block: 0.4rem;
    padding-inline: 0.75rem;
    background: var(--bg2);
    border-block-end: 1px solid var(--line);
    color: var(--fg);
    font-size: 0.8125rem;
  }

  .held button {
    padding-block: 0.1rem;
    padding-inline: 0.5rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
    color: inherit;
    font: inherit;
    font-size: inherit;
    cursor: pointer;
  }

  .held button:hover {
    border-color: var(--acc);
  }

  .error {
    margin: 0;
    padding-block: 0.4rem;
    padding-inline: 0.75rem;
    background: var(--err);
    color: #fff;
    font-size: 0.8125rem;
  }
</style>
