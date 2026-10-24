<script lang="ts">
  /**
   * A footnote or a cross-reference, taken apart.
   *
   * The same three fields wherever it appears: docked in the Workbench's
   * inspector, and floating beside the caret in Study. One component, because
   * a note shown in two places that disagreed about what its caller was would
   * be worse than having only one of them.
   *
   * Reading only, apart from the conversion. Editing `\ft` here would mean
   * writing back into the middle of a sentence, and the sentence is already
   * open in the editor three inches away — the useful thing this offers is
   * seeing the note without hunting for it, and getting to it in one click.
   */

  import type { NoteEntry } from "../lib/notes";

  interface Props {
    notes: NoteEntry[];
    /** Which note is shown. Clamped by the caller. */
    selected: number;
    onselect: (index: number) => void;
    /** Puts the caret at an offset. */
    ongo: (offset: number) => void;
    /** Turns a footnote into a cross-reference, or back. */
    onconvert: (note: NoteEntry) => void;
    readOnly: boolean;
    /** The floating card is tighter and has its own header. */
    compact?: boolean;
  }

  let { notes, selected, onselect, ongo, onconvert, readOnly, compact = false }: Props = $props();

  const note = $derived(notes[selected]);
  const other = $derived(note?.isReference ? "\\f" : "\\x");
</script>

{#if note}
  <div class="note" class:compact>
    <div class="head">
      <code class="marker">\{note.marker}</code>
      <span class="kind">{note.isReference ? "Cross reference" : "Footnote"}</span>
      {#if note.start !== null}<span class="where">at {note.reference ?? "the caret"}</span>{/if}
      <span class="of">{selected + 1} of {notes.length}</span>
      <span class="step">
        <button
          type="button"
          disabled={selected === 0}
          aria-label="Previous note"
          title="Previous note"
          onclick={() => onselect(selected - 1)}>‹</button>
        <button
          type="button"
          disabled={selected >= notes.length - 1}
          aria-label="Next note"
          title="Next note"
          onclick={() => onselect(selected + 1)}>›</button>
      </span>
    </div>

    <dl class="fields">
      <dt>caller</dt>
      <dd>
        {note.caller}
        {#if note.caller === "+"}<span class="hint">(numbered automatically)</span>{/if}
        {#if note.caller === "-"}<span class="hint">(no caller shown)</span>{/if}
      </dd>

      <dt><code>\{note.isReference ? "xo" : "fr"}</code></dt>
      <dd>{note.reference ?? "—"}</dd>

      <dt><code>\{note.isReference ? "xt" : "ft"}</code></dt>
      <dd class="text">{note.text || "—"}</dd>
    </dl>

    <div class="actions">
      <button
        type="button"
        class="secondary"
        disabled={note.start === null}
        onclick={() => note.start !== null && ongo(note.start)}
      >
        Edit in source
      </button>
      <button
        type="button"
        class="ghost"
        disabled={readOnly || note.start === null}
        title="Rewrite this note's markers as {other}"
        onclick={() => onconvert(note)}
      >
        Convert to {other}
      </button>
      {#if compact}
        <span class="escape">Esc to close</span>
      {/if}
    </div>
  </div>
{:else}
  <p class="empty">No footnotes or cross-references in this chapter.</p>
{/if}

<style>
  .note {
    display: flex;
    flex-direction: column;
    font-size: 0.78125rem;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding-block: 0.5625rem;
    padding-inline: 0.75rem;
    border-block-end: 1px solid var(--line);
  }

  .marker {
    font-family: var(--font-mono);
    color: var(--mk);
  }

  .kind {
    font-weight: 600;
  }

  .where,
  .of {
    color: var(--fg3);
    font-size: 0.71875rem;
  }

  .of {
    margin-inline-start: auto;
  }

  .step {
    display: flex;
    gap: 0.125rem;
  }

  .step button {
    inline-size: 1.125rem;
    block-size: 1.125rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--fg3);
    font: inherit;
    cursor: pointer;
  }

  .step button:hover:not(:disabled) {
    background: var(--bg3);
    color: var(--fg);
  }

  .step button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .fields {
    display: grid;
    grid-template-columns: 4rem minmax(0, 1fr);
    row-gap: 0.375rem;
    column-gap: 0.625rem;
    margin: 0;
    padding-block: 0.625rem;
    padding-inline: 0.75rem;
  }

  dt {
    color: var(--fg3);
    font-size: 0.71875rem;
  }

  dt code {
    font-family: var(--font-mono);
  }

  dd {
    margin: 0;
    min-inline-size: 0;
  }

  /* The note's own words, which may be in any script. */
  .text {
    font-family: var(--font-content);
    line-height: 1.5;
  }

  .hint {
    color: var(--fg3);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding-block: 0 0.625rem;
    padding-inline: 0.75rem;
  }

  .secondary,
  .ghost {
    block-size: 1.625rem;
    padding-inline: 0.5625rem;
    border-radius: var(--radius);
    font: inherit;
    font-size: 0.71875rem;
    font-weight: 500;
    cursor: pointer;
  }

  .secondary {
    border: 1px solid var(--line);
    background: none;
    color: var(--fg);
  }

  .secondary:hover:not(:disabled) {
    border-color: var(--acc);
  }

  .ghost {
    border: none;
    background: none;
    color: var(--acc-text);
  }

  .ghost:hover:not(:disabled) {
    text-decoration: underline;
  }

  .secondary:disabled,
  .ghost:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .escape {
    margin-inline-start: auto;
    font-size: 0.6875rem;
    color: var(--fg3);
  }

  .empty {
    margin: 0;
    padding-block: 1rem;
    padding-inline: 0.875rem;
    color: var(--fg3);
    font-size: 0.75rem;
  }
</style>
