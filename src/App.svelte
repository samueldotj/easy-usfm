<script lang="ts">
  import { onDestroy, onMount } from "svelte";

  import Editor from "./components/Editor.svelte";
  import SplitPane from "./components/SplitPane.svelte";
  import Toolbar from "./components/Toolbar.svelte";
  import { doc } from "./lib/document.svelte";
  import { engine } from "./lib/engine.svelte";
  import { isDesktop } from "./lib/shell";

  let editor: Editor | undefined = $state();
  let error = $state<string | null>(null);

  const lines = $derived(doc.text.split("\n").length);
  const counts = $derived(engine.counts);

  onMount(async () => {
    engine.start();
    await run(() => doc.createNew());
    // The editor was constructed before the document existed, so it is given
    // the text explicitly rather than relying on the prop it was mounted with.
    editor?.load(doc.text);
    engine.open(doc.text);
    if (isDesktop()) await guardTheWindow();
  });

  // Separate from onMount, which cannot return a cleanup when it is async.
  onDestroy(() => engine.stop());

  $effect(() => {
    document.title = doc.title;
  });

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

  function onKeyDown(event: KeyboardEvent): void {
    // Ctrl on Windows and Linux, Command on macOS (PRODUCT §7).
    const accel = event.ctrlKey || event.metaKey;

    if (event.key === "F6") {
      event.preventDefault();
      editor?.focus();
      return;
    }
    if (!accel) return;

    switch (event.key.toLowerCase()) {
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
    onsaveas={() => void run(() => doc.saveAs())}
  />

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <main>
    <SplitPane id="main" startLabel="USFM source" endLabel="Preview">
      {#snippet start()}
        <Editor
          bind:this={editor}
          value={doc.text}
          onchange={(text, changes) => {
            doc.edited(text, changes);
            engine.edit(
              changes.map((change) => ({
                from: change.fromA,
                to: change.toA,
                insert: change.inserted,
              })),
              text,
            );
          }}
        />
      {/snippet}

      {#snippet end()}
        <div class="placeholder">
          {#if engine.diagnostics.length === 0}
            <p>No diagnostics.</p>
          {:else}
            <ul class="diagnostics">
              {#each engine.diagnostics.slice(0, 50) as diagnostic (diagnostic.code + diagnostic.start)}
                <li class={diagnostic.severity}>
                  <code>{diagnostic.code}</code>
                  {diagnostic.message}
                </li>
              {/each}
            </ul>
          {/if}
          <p class="hint">The formatted preview arrives with M3.</p>
        </div>
      {/snippet}
    </SplitPane>
  </main>

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
    <div class="spacer"></div>
    {#if engine.desynced}
      <span class="warn" title={engine.desynced}>Engine resyncing</span>
    {:else if engine.diagnostics.length > 0}
      <span>{counts.error} errors, {counts.warning} warnings</span>
    {/if}
    <span>{engine.version ? `Engine ${engine.version}` : "Engine loading…"}</span>
    {#if doc.saveNote}<span class="note">Saved via {doc.saveNote}</span>{/if}
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

  .placeholder {
    display: flex;
    flex-direction: column;
    block-size: 100%;
    overflow: auto;
    padding: 1rem;
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .warn {
    color: #d97706;
  }

  .diagnostics {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .diagnostics li {
    padding-block: 0.3rem;
    border-block-end: 1px solid var(--border);
  }

  .diagnostics code {
    font-family: var(--font-gutter);
    font-size: 0.8em;
    margin-inline-end: 0.5rem;
  }

  .diagnostics .error code {
    color: #dc2626;
  }

  .diagnostics .warning code {
    color: #d97706;
  }

  .hint {
    margin-block-start: auto;
    padding-block-start: 1rem;
    font-size: 0.8125rem;
  }
</style>
