<script lang="ts">
  import { onMount } from "svelte";
  import Editor from "./components/Editor.svelte";
  import SplitPane from "./components/SplitPane.svelte";
  import ThemeSelect from "./components/ThemeSelect.svelte";
  import { engineVersion } from "./lib/shell";

  const SAMPLE = `\\id GEN Genesis
\\h Genesis
\\mt1 The First Book of Moses
\\c 1
\\p
\\v 1 In the beginning God created the heaven and the earth.
\\v 2 And the earth was without form, and void.
`;

  let engine = $state<string | null>(null);
  let source = $state(SAMPLE);
  let editor: Editor | undefined = $state();

  // Counted in UTF-16 code units, which is what CodeMirror and the DOM count
  // in (UNICODE §1). The status bar will show graphemes once there is a cursor
  // to report a column for (P2.9).
  const units = $derived(source.length);
  const lines = $derived(source.split("\n").length);

  onMount(async () => {
    engine = await engineVersion().catch(() => null);
  });

  // F6 cycles pane focus (PRODUCT §7). Only the editor exists to focus today;
  // the preview joins the cycle at P3.1.
  function onKeyDown(event: KeyboardEvent): void {
    if (event.key === "F6") {
      event.preventDefault();
      editor?.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<div class="app">
  <header>
    <span class="title">Easy USFM</span>
    <div class="spacer"></div>
    <ThemeSelect />
  </header>

  <main>
    <SplitPane id="main" startLabel="USFM source" endLabel="Preview">
      {#snippet start()}
        <Editor bind:this={editor} value={source} onchange={(next) => (source = next)} />
      {/snippet}

      {#snippet end()}
        <div class="placeholder">
          <p>The preview arrives with M3.</p>
          <p class="hint">
            Until then this pane holds the space, so the split it has to live in
            is the one everything else is built around.
          </p>
        </div>
      {/snippet}
    </SplitPane>
  </main>

  <footer>
    <span>{lines} lines</span>
    <span>{units} UTF-16 units</span>
    <div class="spacer"></div>
    <span>{engine ? `Engine ${engine}` : "Engine unavailable"}</span>
  </footer>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    block-size: 100%;
  }

  header,
  footer {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding-block: 0.35rem;
    padding-inline: 0.75rem;
    background: var(--surface-sunken);
    font-size: 0.8125rem;
    color: var(--text-muted);
    flex: 0 0 auto;
  }

  header {
    border-block-end: 1px solid var(--border);
  }

  footer {
    border-block-start: 1px solid var(--border);
  }

  .title {
    font-weight: 600;
    color: var(--text);
  }

  .spacer {
    flex: 1 1 auto;
  }

  main {
    flex: 1 1 auto;
    min-block-size: 0;
  }

  .placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    block-size: 100%;
    padding-inline: 2rem;
    text-align: center;
    color: var(--text-muted);
  }

  .hint {
    max-inline-size: 34ch;
    font-size: 0.8125rem;
  }
</style>
