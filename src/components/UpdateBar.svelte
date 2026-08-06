<script lang="ts">
  /**
   * "A new version is ready — Reload" (PRODUCT §12) — P5.1.
   *
   * A bar, never an automatic reload. A service worker that steps forward on
   * its own reloads the page, and in an editor that can happen while somebody
   * is typing into a document they have not saved. The update is already
   * installed and costs nothing to leave waiting; it will still be there
   * tomorrow.
   */

  import { pwa } from "../lib/pwa.svelte";
</script>

{#if pwa.updateReady}
  <p class="bar" role="status">
    <span>A new version is ready.</span>
    <button type="button" onclick={() => pwa.reload()}>Reload</button>
    <button type="button" onclick={() => pwa.dismiss()}>Not now</button>
  </p>
{/if}

<style>
  .bar {
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

  button {
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

  button:hover {
    border-color: var(--accent);
  }
</style>
