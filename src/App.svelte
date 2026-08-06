<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  import DiagnosticsPanel from "./components/DiagnosticsPanel.svelte";
  import Editor from "./components/Editor.svelte";
  import ExternalChange from "./components/ExternalChange.svelte";
  import FindBar from "./components/FindBar.svelte";
  import FontNotice from "./components/FontNotice.svelte";
  import GoToReference from "./components/GoToReference.svelte";
  import Preview from "./components/preview/Preview.svelte";
  import PrintSettings from "./components/PrintSettings.svelte";
  import RecoveryPrompt from "./components/RecoveryPrompt.svelte";
  import SplitPane from "./components/SplitPane.svelte";
  import Toolbar from "./components/Toolbar.svelte";
  import VersionPicker from "./components/VersionPicker.svelte";
  import { doc } from "./lib/document.svelte";
  import type { FileChanged } from "./lib/documentService";
  import { engine } from "./lib/engine.svelte";
  import { fonts } from "./lib/fonts.svelte";
  import { figures } from "./lib/figures.svelte";
  import { hasInvisibles } from "./lib/invisibles";
  import { print } from "./lib/print.svelte";
  import { ScrollSync, elementFor, scrollTo, topmostOffset, type Pane } from "./lib/scrollsync";
  import { SnapshotSchedule } from "./lib/snapshots";
  import { isDesktop } from "./lib/shell";
  import { theme, type Theme } from "./lib/theme.svelte";

  let editor: Editor | undefined = $state();
  let goto: GoToReference | undefined = $state();
  let find: FindBar | undefined = $state();
  let preview: Preview | undefined = $state();
  let printSettings: PrintSettings | undefined = $state();
  let recoveryPrompt: RecoveryPrompt | undefined = $state();

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
  let panelOpen = $state(true);
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

  const lines = $derived(doc.text.split("\n").length);
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

  const sync = new ScrollSync();

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
  /** The host's limitations, as one tooltip. Blank-line separated to read. */
  const limitations = $derived(doc.limitations.join("\n\n"));

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

    if (!isDesktop()) guardTheTab();

    if (isDesktop()) {
      await guardTheWindow();
      await listenToMenu();
      // The shell starts with an empty Open Recent; the list lives here.
      await doc.pushRecentToMenu();
    }
  });

  /**
   * The native menu bar is another way to ask for the same commands.
   *
   * One listener with the item's id as the payload, so adding an item to the
   * menu does not mean adding a listener here — and so the menu can never
   * drift into being a second implementation of anything.
   */
  async function listenToMenu(): Promise<void> {
    const { listen } = await import("@tauri-apps/api/event");

    await listen<string>("menu", async (event) => {
      const id = event.payload;

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
        case "focus-editor":
          cyclePanes(true);
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
          panelOpen = !panelOpen;
          break;
        case "next-diagnostic":
          editor?.step(true);
          break;
        case "previous-diagnostic":
          editor?.step(false);
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
      }
    });
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
      panelOpen = !panelOpen;
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

<div class="app">
  <Toolbar
    onnew={() => void load(() => doc.createNew())}
    onopen={(path) => void load(() => doc.open(path))}
    onsave={() => void saved(() => doc.save())}
  />

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

  <main>
    <SplitPane id="main" startLabel="USFM source" endLabel="Preview">
      {#snippet start()}
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
            engine.locate(at);
          }}
          oncomplete={(at) => engine.completions(at)}
          {showInvisibles}
          readOnly={doc.readOnly}
          onscroll={editorScrolled}
        />
      {/snippet}

      {#snippet end()}
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
      {/snippet}
    </SplitPane>
  </main>

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

  <GoToReference
    bind:this={goto}
    onsubmit={goToReference}
    onclose={() => editor?.focus()}
  />

  <DiagnosticsPanel
    diagnostics={engine.diagnostics}
    open={panelOpen}
    ontoggle={() => (panelOpen = !panelOpen)}
    onselect={(index, focus) => editor?.goTo(index, focus)}
    onescape={() => editor?.focus()}
  />

  <footer>
    <span>{lines} lines</span>
    <span>{doc.text.length} UTF-16 units</span>
    {#if doc.summary}
      <span>{doc.summary.encoding}</span>
      <span title={doc.summary.mixed_eol ? "This file mixes line endings" : ""}>
        {doc.summary.eol}
      </span>
      {#if doc.summary.bom}<span>BOM</span>{/if}
    {/if}
    {#if engine.reference}
      <!-- Where the cursor is. Shows the published number when the file has
           one, because that is the number on the page (PRODUCT §6.2). -->
      <span class="reference">{engine.reference}</span>
    {/if}
    <VersionPicker version={engine.usfm} onchange={(v) => engine.overrideVersion(v)} />
    <div class="spacer"></div>
    {#if engine.desynced}
      <span class="warn" title={engine.desynced}>Engine resyncing</span>
    {:else if engine.diagnostics.length > 0}
      <span>{counts.error} errors, {counts.warning} warnings</span>
    {/if}
    <span>{engine.version ? `Engine ${engine.version}` : "Engine loading…"}</span>
    {#if doc.saveNote}<span class="note">Saved via {doc.saveNote}</span>{/if}
    <!-- FILE-FIDELITY 3: a clean reload is silent apart from "a transient
         status-bar notice". Here rather than in a bar, because nothing is
         being asked and nothing was lost. -->
    {#if reloaded}<span class="note">{reloaded}</span>{/if}
    {#if doc.limitations.length > 0}
      <!-- What this host cannot do, said plainly. An editor that appears to
           save and does not is the worst failure available to it, so the
           browser build says so before it is relied on. -->
      <!-- One label whatever the host can do, because the alternative
           overclaims: a *new* document has no handle yet even in a browser
           that can save in place, so keying the wording on `savesInPlace`
           says "downloads a copy" about a Save that is about to write a real
           file. The tooltip carries what is actually true. -->
      <span class="limits" title={limitations}>Browser limits</span>
    {/if}
    <span>{doc.dirty ? "Unsaved changes" : "Saved"}</span>
  </footer>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    block-size: 100%;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding-block: 0.3rem;
    padding-inline: 0.75rem;
    background: var(--surface-sunken);
    border-block-start: 1px solid var(--border);
    font-size: 0.8125rem;
    color: var(--text-muted);
    flex: 0 0 auto;
  }

  .spacer {
    flex: 1 1 auto;
  }

  .note {
    color: var(--accent);
  }

  .limits {
    color: var(--severity-warning);
    cursor: help;
  }

  .reference {
    color: var(--text);
    /* The reference may carry a published number in any script (UNICODE §6). */
    font-family: var(--font-content);
    font-variant-numeric: tabular-nums;
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
    background: var(--surface-sunken);
    border-block-end: 1px solid var(--border);
    color: var(--text);
    font-size: 0.8125rem;
  }

  .held button {
    padding-block: 0.1rem;
    padding-inline: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface);
    color: inherit;
    font: inherit;
    font-size: inherit;
    cursor: pointer;
  }

  .held button:hover {
    border-color: var(--accent);
  }

  .error {
    margin: 0;
    padding-block: 0.4rem;
    padding-inline: 0.75rem;
    background: #7f1d1d;
    color: #fff;
    font-size: 0.8125rem;
  }

  main {
    flex: 1 1 auto;
    min-block-size: 0;
  }

  .warn {
    color: #d97706;
  }
</style>
