<script lang="ts">
  /**
   * What is open, and what can be done to it. The Workbench's second row.
   *
   * Three things sit here and each earns its place: the file's identity, the
   * facts about it that would otherwise need a menu (its USFM version, its
   * encoding, whether it has errors), and the four controls someone touches
   * every few minutes.
   *
   * The book and the translation are read out of the file's own header rather
   * than out of the file *name*, which is the difference between "John · Berean
   * Standard Bible" and "43_JHNBSB.usfm". Both are shown; the name is the one
   * that is unambiguous and it is set in the monospace face to say so.
   */

  import { identityOf } from "../lib/identity";
  import { layout } from "../lib/layout.svelte";
  import { doc } from "../lib/document.svelte";
  import Segmented from "./Segmented.svelte";

  interface Props {
    /** Runs a command by id. */
    onrun: (id: string) => void;
    /** The document's USFM version, as the engine reports it. */
    usfm: string;
    /** How many errors and warnings, for the chip. */
    errors: number;
    warnings: number;
    /** Whether the engine has finished starting, so Validate can wait for it. */
    ready: boolean;
  }

  let { onrun, usfm, errors, warnings, ready }: Props = $props();

  const identity = $derived(identityOf(doc.text));

  /** The encoding and terminator, as one chip. */
  const encoding = $derived.by(() => {
    const summary = doc.summary;
    if (!summary) return null;
    return `${summary.encoding} · ${summary.eol}${summary.bom ? " · BOM" : ""}`;
  });

  const problems = $derived.by(() => {
    if (errors > 0) return { text: `${errors} ${errors === 1 ? "error" : "errors"}`, bad: true };
    if (warnings > 0) {
      return { text: `${warnings} ${warnings === 1 ? "warning" : "warnings"}`, bad: false };
    }
    return null;
  });

  const panes = [
    { value: "editor" as const, label: "Editor", title: "Editor only" },
    { value: "split" as const, label: "Split", title: "Editor and preview" },
    { value: "preview" as const, label: "Preview", title: "Preview only" },
  ];
</script>

<div class="bar">
  <div class="who">
    {#if identity.book}
      <span class="book">{identity.book}</span>
    {/if}
    {#if identity.translation}
      <span class="translation">{identity.translation}</span>
    {/if}
    <span class="file" title={doc.path ?? "Not saved yet"}>{doc.name}</span>
    {#if doc.dirty}
      <span class="dot" title="Unsaved changes" aria-label="Unsaved changes"></span>
    {/if}
  </div>

  <div class="chips">
    <span class="chip">USFM {usfm}</span>
    {#if encoding}<span class="chip">{encoding}</span>{/if}
    {#if problems}
      <button
        type="button"
        class="chip problems"
        class:bad={problems.bad}
        onclick={() => onrun("toggle-diagnostics")}
      >
        {problems.text}
      </button>
    {/if}
  </div>

  <div class="controls">
    <!--
      A switch rather than a checkbox with a tick: it turns a behaviour on and
      off rather than selecting something, which is exactly the distinction
      `role="switch"` exists for.
    -->
    <button
      type="button"
      class="switch"
      role="switch"
      aria-checked={layout.sync}
      onclick={() => layout.toggleSync()}
    >
      <span class="track" class:on={layout.sync}><span class="knob"></span></span>
      Sync scroll
    </button>

    <Segmented
      name="panes"
      label="Panes"
      options={panes}
      value={layout.panes}
      onchange={(value) => layout.setPanes(value)}
    />

    <!--
      Validation is continuous, so this is not "check now" -- it is "show me
      what the check found", which is the diagnostics table.
    -->
    <button type="button" class="secondary" disabled={!ready} onclick={() => onrun("toggle-diagnostics")}>
      Validate
    </button>

    <button
      type="button"
      class="primary"
      disabled={!doc.dirty && doc.path !== null}
      onclick={() => onrun("save")}
    >
      Save
      <span class="keys">Ctrl S</span>
    </button>
  </div>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding-block: 0.4rem;
    padding-inline: 1rem;
    border-block-end: 1px solid var(--line);
    flex: 0 0 auto;
    font-size: 0.8125rem;
    min-block-size: 2.875rem;
  }

  .who {
    display: flex;
    align-items: baseline;
    gap: 0.625rem;
    min-inline-size: 0;
  }

  .book {
    font-weight: 600;
    font-size: 0.9375rem;
    white-space: nowrap;
  }

  .translation {
    color: var(--fg2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .file {
    font-family: var(--font-mono);
    font-size: 0.71875rem;
    color: var(--fg3);
    white-space: nowrap;
  }

  .dot {
    inline-size: 0.4375rem;
    block-size: 0.4375rem;
    border-radius: 50%;
    background: var(--acc);
    align-self: center;
    flex: 0 0 auto;
  }

  .chips {
    display: flex;
    gap: 0.375rem;
    flex: 0 0 auto;
  }

  .chip {
    padding-block: 0.125rem;
    padding-inline: 0.4375rem;
    background: var(--bg3);
    border: none;
    border-radius: var(--radius-sm);
    color: var(--fg2);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    white-space: nowrap;
  }

  .problems {
    cursor: pointer;
    color: var(--acc-text);
  }

  .problems.bad {
    color: var(--err);
  }

  .problems:hover {
    background: var(--hl);
  }

  .controls {
    margin-inline-start: auto;
    display: flex;
    align-items: center;
    gap: 0.875rem;
    flex: 0 0 auto;
  }

  .switch {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border: none;
    background: none;
    color: var(--fg2);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
    padding: 0;
  }

  .switch:hover {
    color: var(--fg);
  }

  .track {
    position: relative;
    inline-size: 1.875rem;
    block-size: 1rem;
    border-radius: 0.5rem;
    background: var(--line);
    display: inline-block;
    flex: 0 0 auto;
    transition: background 120ms;
  }

  .track.on {
    background: var(--acc);
  }

  .knob {
    position: absolute;
    inset-block-start: 0.125rem;
    inset-inline-start: 0.125rem;
    inline-size: 0.75rem;
    block-size: 0.75rem;
    border-radius: 50%;
    background: var(--bg);
    transition: inset-inline-start 120ms;
  }

  .track.on .knob {
    inset-inline-start: 0.9375rem;
    background: var(--acc-ink);
  }

  .secondary,
  .primary {
    block-size: 1.875rem;
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding-inline: 0.75rem;
    border-radius: var(--radius);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .secondary {
    border: 1px solid var(--line);
    background: none;
    color: var(--fg);
    font-weight: 500;
  }

  .secondary:hover:not(:disabled) {
    border-color: var(--acc);
  }

  .primary {
    border: none;
    background: var(--acc-fill);
    color: var(--acc-ink);
    font-weight: 600;
  }

  .primary:disabled,
  .secondary:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .keys {
    font-family: var(--font-mono);
    font-weight: 400;
    font-size: 0.65625rem;
    opacity: 0.75;
  }
</style>
