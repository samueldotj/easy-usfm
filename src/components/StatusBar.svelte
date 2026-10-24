<script lang="ts">
  /**
   * The bottom line: everything true about the document that is one number.
   *
   * The redesign sets it in the monospace face and gives it more to say —
   * words as well as lines, the caret's line and column, the reference. All of
   * it is either already known or cheap, with one exception.
   *
   * # The word count is deferred
   *
   * Counting words means walking the whole document, and the whole document
   * can be two megabytes. Doing that on every keystroke would put a linear
   * scan on the typing path, which is exactly what ARCHITECTURE keeps off it.
   * So it settles: the count is recomputed once typing stops, and until then
   * it shows the last one it had. A number that is a second stale is a fair
   * trade for a number that is free.
   */

  import { doc } from "../lib/document.svelte";
  import { engine } from "../lib/engine.svelte";
  import { zoom } from "../lib/zoom.svelte";
  import VersionPicker from "./VersionPicker.svelte";

  interface Props {
    /** The caret, as the editor reports it. */
    position: { line: number; column: number } | null;
    /** A transient message — "Reloaded from disk". */
    notice: string | null;
    errors: number;
    warnings: number;
  }

  let { position, notice, errors, warnings }: Props = $props();

  /** How long typing has to stop before the document is counted. */
  const SETTLE_MS = 500;

  const lines = $derived(doc.text.split("\n").length);

  let words = $state(0);
  $effect(() => {
    const text = doc.text;
    const timer = setTimeout(() => {
      words = text.split(/\s+/).filter(Boolean).length;
    }, SETTLE_MS);
    return () => clearTimeout(timer);
  });

  const number = (value: number) => value.toLocaleString();

  const limitations = $derived(doc.limitations.join("\n\n"));
</script>

<footer>
  <span>{number(lines)} lines</span>
  <span>{number(words)} words</span>

  {#if engine.reference}
    <!-- Where the cursor is. Shows the published number when the file has one,
         because that is the number on the page (PRODUCT §6.2). -->
    <span class="reference">{engine.reference}</span>
  {/if}

  {#if position}
    <span>Ln {position.line}, Col {position.column}</span>
  {/if}

  {#if doc.summary}
    <span title={doc.summary.mixed_eol ? "This file mixes line endings" : ""}>
      {doc.summary.encoding} · {doc.summary.eol}{doc.summary.bom ? " · BOM" : ""}
    </span>
  {/if}

  <VersionPicker version={engine.usfm} onchange={(version) => engine.overrideVersion(version)} />

  <div class="spacer"></div>

  {#if engine.desynced}
    <span class="warn" title={engine.desynced}>Engine resyncing</span>
  {:else if errors > 0 || warnings > 0}
    <span class="problems" class:bad={errors > 0}>
      <span class="dot" aria-hidden="true"></span>
      {errors} · {warnings}
      <span class="visually-hidden">
        {errors}
        {errors === 1 ? "error" : "errors"}, {warnings}
        {warnings === 1 ? "warning" : "warnings"}
      </span>
    </span>
  {/if}

  <span>{engine.version ? `Engine ${engine.version}` : "Engine loading…"}</span>

  {#if doc.saveNote}<span class="note">Saved via {doc.saveNote}</span>{/if}

  <!-- FILE-FIDELITY §3: a clean reload is silent apart from "a transient
       status-bar notice". Here rather than in a bar, because nothing is being
       asked and nothing was lost. -->
  {#if notice}<span class="note">{notice}</span>{/if}

  <!-- Only when zoomed. A permanent "100%" is a number nobody reads, and its
       absence is what makes the reading mean something when it appears. -->
  {#if zoom.changed}
    <button type="button" class="zoom" title="Reset to actual size" onclick={() => zoom.reset()}>
      {zoom.percent}%
    </button>
  {/if}

  {#if doc.limitations.length > 0}
    <!-- What this host cannot do, said plainly. An editor that appears to save
         and does not is the worst failure available to it, so the browser
         build says so before it is relied on. -->
    <span class="limits" title={limitations}>Browser limits</span>
  {/if}

  <span class:unsaved={doc.dirty}>{doc.dirty ? "Unsaved" : "Saved"}</span>
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    gap: 1.125rem;
    padding-block: 0.25rem;
    padding-inline: 1rem;
    background: var(--bg2);
    border-block-start: 1px solid var(--line);
    font-family: var(--font-mono);
    font-size: 0.71875rem;
    color: var(--fg2);
    flex: 0 0 auto;
    white-space: nowrap;
    overflow-x: auto;
  }

  .spacer {
    flex: 1 1 auto;
    min-inline-size: 0.5rem;
  }

  .reference {
    color: var(--fg);
    font-weight: 500;
    /* The reference may carry a published number in any script (UNICODE §6). */
    font-family: var(--font-content);
    font-variant-numeric: tabular-nums;
  }

  .problems {
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  .dot {
    inline-size: 0.4375rem;
    block-size: 0.4375rem;
    border-radius: 50%;
    background: var(--acc);
    display: inline-block;
  }

  .problems.bad .dot {
    background: var(--err);
  }

  .note,
  .unsaved {
    color: var(--acc-text);
  }

  .limits {
    color: var(--severity-warning);
    cursor: help;
  }

  .warn {
    color: var(--severity-warning);
  }

  .zoom {
    padding-block: 0;
    padding-inline: 0.35rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: none;
    color: inherit;
    font: inherit;
    font-size: inherit;
    cursor: pointer;
  }

  .zoom:hover {
    border-color: var(--acc);
  }
</style>
