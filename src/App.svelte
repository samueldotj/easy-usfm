<script lang="ts">
  import { onMount } from "svelte";

  import Editor from "./components/Editor.svelte";
  import SplitPane from "./components/SplitPane.svelte";
  import Toolbar from "./components/Toolbar.svelte";
  import { doc } from "./lib/document.svelte";
  import { isDesktop } from "./lib/shell";

  let editor: Editor | undefined = $state();
  let error = $state<string | null>(null);

  const lines = $derived(doc.text.split("\n").length);

  onMount(async () => {
    await run(() => doc.createNew());
    if (isDesktop()) await guardTheWindow();
  });

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
          onchange={(text, changes) => doc.edited(text, changes)}
        />
      {/snippet}

      {#snippet end()}
        <div class="placeholder">
          <p>The preview arrives with M3.</p>
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
    align-items: center;
    justify-content: center;
    block-size: 100%;
    color: var(--text-muted);
    font-size: 0.875rem;
  }
</style>
