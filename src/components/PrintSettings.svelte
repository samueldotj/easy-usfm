<script lang="ts">
  /**
   * The print panel (PRODUCT §8) — P3.11.
   *
   * Opened by Ctrl+P rather than printing straight away, because every one of
   * these settings changes what comes out of the printer and the browser's own
   * dialog cannot ask about any of them. Print is the button at the end.
   *
   * The footnote limitation is stated here, in the panel, where the choice is
   * made. PRODUCT §8 asks for exactly that: per-page footnotes need
   * `float: footnote`, which no browser implements, and a reader choosing where
   * notes go deserves to know why the obvious option is missing rather than
   * wondering whether they have overlooked it.
   */

  import { print } from "../lib/print.svelte";

  interface Props {
    /** Whether the document has a path, which is what settings are stored per. */
    saved: boolean;
  }

  let { saved }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  export function open(): void {
    dialog?.showModal();
  }

  const settings = $derived(print.current);
</script>

<dialog bind:this={dialog} aria-label="Print settings">
  <form
    method="dialog"
    onsubmit={(event) => {
      // The default submit closes the dialog, which would tear the preview out
      // from under the print renderer.
      event.preventDefault();
      dialog?.close();
      print.print();
    }}
  >
    <h2>Print</h2>

    <div class="grid">
      <label>
        Paper
        <select
          value={settings.size}
          onchange={(event) => print.set("size", event.currentTarget.value as "a4" | "letter")}
        >
          <option value="a4">A4</option>
          <option value="letter">Letter</option>
        </select>
      </label>

      <label>
        Base size
        <span class="with-unit">
          <input
            type="number"
            min="6"
            max="24"
            step="0.5"
            value={settings.fontSize}
            onchange={(event) => print.set("fontSize", event.currentTarget.valueAsNumber)}
          />
          <span class="unit">pt</span>
        </span>
      </label>

      <label>
        Outer margin
        <span class="with-unit">
          <input
            type="number"
            min="0"
            max="60"
            step="1"
            value={settings.marginOuter}
            onchange={(event) => print.set("marginOuter", event.currentTarget.valueAsNumber)}
          />
          <span class="unit">mm</span>
        </span>
      </label>

      <label>
        Inner margin
        <span class="with-unit">
          <input
            type="number"
            min="0"
            max="60"
            step="1"
            value={settings.marginInner}
            onchange={(event) => print.set("marginInner", event.currentTarget.valueAsNumber)}
          />
          <span class="unit">mm</span>
        </span>
      </label>
    </div>

    <fieldset>
      <legend>Notes</legend>
      <label class="radio">
        <input
          type="radio"
          name="notes"
          checked={settings.notes === "chapter"}
          onchange={() => print.set("notes", "chapter")}
        />
        At the end of each chapter
      </label>
      <label class="radio">
        <input
          type="radio"
          name="notes"
          checked={settings.notes === "document"}
          onchange={() => print.set("notes", "document")}
        />
        At the end of the document
      </label>
      <p class="note">
        Footnotes at the bottom of each page are not possible: no browser
        implements the CSS feature they need. Easy USFM is not a typesetter —
        for laid-out Scripture, use PTXprint.
      </p>
    </fieldset>

    <fieldset>
      <legend>Include</legend>
      <label class="check">
        <input
          type="checkbox"
          checked={settings.sectionHeadings}
          onchange={(event) => print.set("sectionHeadings", event.currentTarget.checked)}
        />
        Section headings
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings.introduction}
          onchange={(event) => print.set("introduction", event.currentTarget.checked)}
        />
        Introduction material
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings.crossReferences}
          onchange={(event) => print.set("crossReferences", event.currentTarget.checked)}
        />
        Cross-references
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={settings.chapterStartsPage}
          onchange={(event) => print.set("chapterStartsPage", event.currentTarget.checked)}
        />
        Start each chapter on a new page
      </label>
    </fieldset>

    {#if !saved}
      <p class="note">
        These settings are kept per document. This one has not been saved yet,
        so they will be forgotten when it closes.
      </p>
    {/if}

    <div class="actions">
      <button type="button" onclick={() => dialog?.close()}>Cancel</button>
      <button type="submit" class="primary">Print…</button>
    </div>
  </form>
</dialog>

<style>
  dialog {
    inline-size: min(30rem, 90vw);
    padding: 1rem 1.25rem 1.25rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 40%);
  }

  h2 {
    margin-block: 0 0.75rem;
    font-size: 1.05rem;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.6rem 1rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.9rem;
  }

  .with-unit {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
  }

  .unit {
    color: var(--text-muted);
    font-size: 0.85em;
  }

  input[type="number"],
  select {
    inline-size: 100%;
    padding: 0.25rem 0.35rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-sunken);
    color: var(--text);
    font: inherit;
  }

  fieldset {
    margin-block-start: 1rem;
    padding: 0.5rem 0.75rem 0.75rem;
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  legend {
    padding-inline: 0.35rem;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .radio,
  .check {
    flex-direction: row;
    align-items: center;
    gap: 0.45rem;
  }

  .note {
    margin-block: 0.6rem 0;
    color: var(--text-muted);
    font-size: 0.85rem;
    line-height: 1.45;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-block-start: 1.1rem;
  }

  .actions button {
    padding: 0.3rem 0.9rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-sunken);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }

  /* Outlined rather than filled. The accent is a mid blue in light mode and a
     pale blue in dark, and there is no paired foreground token -- so a filled
     button would be legible in one theme and marginal in the other. */
  .actions .primary {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
</style>
