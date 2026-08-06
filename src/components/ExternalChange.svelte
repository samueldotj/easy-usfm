<script lang="ts">
  /**
   * The file changed on disk — FILE-FIDELITY §3, P4.4.
   *
   * "**Dirty** — non-modal bar: *'This file changed on disk.'* with **Reload
   * (discard my changes)** / **Keep my version** / **Compare**. Never a
   * blocking modal; never an automatic overwrite."
   *
   * Non-modal is the requirement and the reason. A modal here interrupts
   * someone mid-sentence over something that has not damaged their work and
   * needs no immediate answer — and a modal that appears while typing gets
   * dismissed by the next keystroke, which is the worst of both.
   *
   * The clean case never reaches this component: §3 reloads silently and says
   * so in the status bar, because there is nothing to lose and nothing to ask.
   */

  interface Props {
    /** What happened: an edit, or the file going away. */
    kind: "external" | "gone";
    onreload: () => void;
    onkeep: () => void;
    oncompare: () => void;
  }

  let { kind, onreload, onkeep, oncompare }: Props = $props();
</script>

<p class="bar" role="status">
  {#if kind === "gone"}
    <span>The file no longer exists — Save will recreate it.</span>
    <button type="button" onclick={onkeep}>Dismiss</button>
  {:else}
    <span>This file changed on disk.</span>
    <button type="button" onclick={onreload}>Reload (discard my changes)</button>
    <button type="button" onclick={onkeep}>Keep my version</button>
    <button type="button" onclick={oncompare}>Compare</button>
  {/if}
</p>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    align-items: center;
    margin: 0;
    padding-block: 0.4rem;
    padding-inline: 0.75rem;
    background: #78350f;
    color: #fff;
    font-size: 0.8125rem;
  }

  button {
    padding-block: 0.1rem;
    padding-inline: 0.5rem;
    border: 1px solid rgb(255 255 255 / 45%);
    border-radius: 4px;
    background: none;
    color: inherit;
    font: inherit;
    font-size: inherit;
    cursor: pointer;
  }

  button:hover {
    background: rgb(255 255 255 / 12%);
  }
</style>
