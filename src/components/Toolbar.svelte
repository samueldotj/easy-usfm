<script lang="ts">
  import { doc } from "../lib/document.svelte";
  import ThemeSelect from "./ThemeSelect.svelte";

  interface Props {
    onnew: () => void;
    onopen: (path?: string) => void;
    onsave: () => void;
    onsaveas: () => void;
  }

  let { onnew, onopen, onsave, onsaveas }: Props = $props();
  let recentOpen = $state(false);

  const shortName = (path: string) => path.split(/[/\\]/).pop() ?? path;
</script>

<!--
  A compact toolbar (PRODUCT §4). Native menus per platform are P6.1; these are
  the same commands, reachable now, with the platform shortcuts already bound
  in App.svelte so the muscle memory is correct from the start.
-->
<header>
  <span class="name" title={doc.path ?? "Not saved yet"}>
    {doc.name}{#if doc.dirty}<span class="dirty" aria-label="Unsaved changes">•</span>{/if}
  </span>

  <nav aria-label="File">
    <button onclick={onnew}>New</button>
    <button onclick={() => onopen()}>Open…</button>

    <div class="recent">
      <button
        aria-haspopup="menu"
        aria-expanded={recentOpen}
        disabled={doc.recent.length === 0}
        onclick={() => (recentOpen = !recentOpen)}
      >
        Recent
      </button>
      {#if recentOpen}
        <ul role="menu">
          {#each doc.recent as path (path)}
            <li role="none">
              <button
                role="menuitem"
                title={path}
                onclick={() => {
                  recentOpen = false;
                  onopen(path);
                }}
              >
                {shortName(path)}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <button onclick={onsave} disabled={!doc.dirty && doc.path !== null}>Save</button>
    <button onclick={onsaveas}>Save As…</button>
  </nav>

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
    max-inline-size: 22ch;
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

  .recent {
    position: relative;
  }

  ul {
    position: absolute;
    inset-block-start: calc(100% + 2px);
    inset-inline-start: 0;
    z-index: 10;
    margin: 0;
    padding: 0.2rem;
    list-style: none;
    min-inline-size: 14rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.18);
  }

  ul button {
    inline-size: 100%;
    text-align: start;
  }

  .spacer {
    flex: 1 1 auto;
  }
</style>
