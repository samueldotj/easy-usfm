<script lang="ts">
  import { doc } from "../lib/document.svelte";
  import { isDesktop } from "../lib/shell";
  import ThemeSelect from "./ThemeSelect.svelte";

  interface Props {
    onnew: () => void;
    onopen: (path?: string) => void;
    onsave: () => void;
  }

  let { onnew, onopen, onsave }: Props = $props();

  // On the desktop the file commands live in the native menu, where someone
  // who has used Windows for twenty years looks for them (PRODUCT §4). The
  // toolbar keeps only what a menu cannot show at a glance: which file is
  // open, and whether it has unsaved work.
  //
  // The browser build has no menu bar to put them in, so it keeps the buttons.
  const desktop = isDesktop();
</script>

<header>
  <span class="name" title={doc.path ?? "Not saved yet"}>
    {doc.name}{#if doc.dirty}<span class="dirty" aria-label="Unsaved changes">•</span>{/if}
  </span>

  {#if !desktop}
    <nav aria-label="File">
      <button onclick={onnew}>New</button>
      <button onclick={() => onopen()}>Open…</button>
      <button onclick={onsave} disabled={!doc.dirty && doc.path !== null}>Save</button>
    </nav>
  {/if}

  <div class="spacer"></div>
  <ThemeSelect />
</header>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding-block: 0.3rem;
    padding-inline: 0.75rem;
    background: var(--surface-sunken);
    border-block-end: 1px solid var(--border);
    font-size: 0.8125rem;
    flex: 0 0 auto;
  }

  .name {
    font-weight: 600;
    color: var(--text);
    max-inline-size: 40ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dirty {
    color: var(--accent);
    margin-inline-start: 0.25rem;
  }

  nav {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  button {
    font: inherit;
    color: var(--text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    padding-block: 0.15rem;
    padding-inline: 0.5rem;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: var(--border);
    background: var(--surface);
  }

  button:disabled {
    color: var(--text-muted);
    cursor: default;
  }

  .spacer {
    flex: 1 1 auto;
  }
</style>
