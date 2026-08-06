<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  import DiagnosticsPanel from "./components/DiagnosticsPanel.svelte";
  import Editor from "./components/Editor.svelte";
  import FindBar from "./components/FindBar.svelte";
  import FontNotice from "./components/FontNotice.svelte";
  import GoToReference from "./components/GoToReference.svelte";
  import Preview from "./components/preview/Preview.svelte";
  import SplitPane from "./components/SplitPane.svelte";
  import Toolbar from "./components/Toolbar.svelte";
  import VersionPicker from "./components/VersionPicker.svelte";
  import { doc } from "./lib/document.svelte";
  import { engine } from "./lib/engine.svelte";
  import { fonts } from "./lib/fonts.svelte";
  import { hasInvisibles } from "./lib/invisibles";
  import { isDesktop } from "./lib/shell";
  import { theme, type Theme } from "./lib/theme.svelte";

  let editor: Editor | undefined = $state();
  let goto: GoToReference | undefined = $state();
  let find: FindBar | undefined = $state();
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

  const counts = $derived(engine.counts);
  /** The host's limitations, as one tooltip. Blank-line separated to read. */
  const limitations = $derived(doc.limitations.join("\n\n"));

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
          await run(() => doc.save());
          break;
        case "save-as":
          await run(() => doc.saveAs());
          break;
        case "focus-editor":
          cyclePanes(true);
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
  onDestroy(() => engine.stop());

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
    await run(action);
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
      if (!(await doc.confirmDiscard())) event.preventDefault();
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
        void run(() => (event.shiftKey ? doc.saveAs() : doc.save()));
        break;
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<div class="app">
  <Toolbar
    onnew={() => void load(() => doc.createNew())}
    onopen={(path) => void load(() => doc.open(path))}
    onsave={() => void run(() => doc.save())}
  />

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
          oncursor={(at) => engine.locate(at)}
          oncomplete={(at) => engine.completions(at)}
          {showInvisibles}
        />
      {/snippet}

      {#snippet end()}
        <Preview
          chunks={engine.chunks}
          previews={engine.previews}
          onselect={(start, end) => editor?.reveal(start, end, false)}
          onfollow={(href) => void followLink(href)}
          onreference={(reference) => void goToReference(reference)}
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
