<script lang="ts">
  import { onMount } from "svelte";
  import { engineVersion } from "./lib/shell";

  // P1.1 is an empty window that proves the shell builds and can talk to the
  // Rust side. The editor arrives with P1.2, and the document lifecycle with
  // P1.10; nothing here is meant to survive them.
  let engine = $state<string | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      engine = await engineVersion();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    }
  });
</script>

<main>
  <h1>Easy USFM</h1>
  <p class="tagline">An editor for individual USFM Scripture files.</p>

  <p class="status">
    {#if error}
      <span class="error">The engine did not answer: {error}</span>
    {:else if engine}
      Engine {engine}
    {:else}
      Starting…
    {/if}
  </p>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    block-size: 100%;
    padding-inline: 2rem;
    text-align: center;
  }

  h1 {
    margin-block: 0 0.25rem;
    font-size: 1.75rem;
    font-weight: 600;
  }

  .tagline {
    margin-block: 0;
    color: var(--text-muted);
  }

  .status {
    margin-block-start: 2rem;
    padding-block: 0.4rem;
    padding-inline: 0.8rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface-sunken);
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .error {
    color: #dc2626;
  }
</style>
