<script lang="ts">
  /**
   * The diagnostics list, collapsible, below the editor (PRODUCT §4).
   *
   * A listbox rather than a stack of buttons. Five hundred diagnostics as five
   * hundred buttons is five hundred tab stops between the editor and the
   * status bar, which is the kind of thing that technically passes an
   * accessibility audit and is unusable in practice (PRODUCT §10). One tab
   * stop, arrow keys within.
   */

  import type { Diagnostic } from "../worker/protocol";

  interface Props {
    diagnostics: Diagnostic[];
    open: boolean;
    ontoggle: () => void;
    /** Move the cursor there. `focus` also moves focus into the editor. */
    onselect: (index: number, focus: boolean) => void;
    /** Escape releases focus, per PRODUCT §10. */
    onescape: () => void;
  }

  let { diagnostics, open, ontoggle, onselect, onescape }: Props = $props();

  /**
   * How many are rendered.
   *
   * A malformed 2 MB file can produce thousands, and a list nobody will scroll
   * to the end of is not worth the layout cost of building. The count above
   * the list is the honest number; this is only what is drawn.
   */
  const SHOWN = 200;

  const GLYPH = { error: "✕", warning: "▲", information: "●" } as const;
  const LABEL = { error: "Error", warning: "Warning", information: "Information" } as const;

  const shown = $derived(diagnostics.slice(0, SHOWN));
  const counts = $derived.by(() => {
    const counts = { error: 0, warning: 0, information: 0 };
    for (const diagnostic of diagnostics) counts[diagnostic.severity] += 1;
    return counts;
  });

  const summary = $derived(
    diagnostics.length === 0
      ? "No diagnostics"
      : `${plural(counts.error, "error")}, ${plural(counts.warning, "warning")}, ` +
        `${plural(counts.information, "information message")}`,
  );

  function plural(count: number, noun: string): string {
    return `${count} ${noun}${count === 1 ? "" : "s"}`;
  }

  /** Roving tabindex. The list is one tab stop; this is the position within it. */
  let active = $state(0);
  let list = $state<HTMLUListElement>();

  // Clamped rather than reset, so a diagnostic disappearing from the middle of
  // the list does not throw away the reader's place.
  $effect(() => {
    if (active >= shown.length) active = Math.max(0, shown.length - 1);
  });

  /**
   * Counts announce on a delay (PRODUCT §10).
   *
   * Diagnostics change on every parse, which is every few keystrokes. Without
   * the delay a screen reader interrupts itself continuously while typing and
   * says nothing usable — the announcement has to describe where the document
   * settled, not every state it passed through.
   */
  let announced = $state("");

  $effect(() => {
    const settled = summary;
    const timer = setTimeout(() => (announced = settled), 1000);
    return () => clearTimeout(timer);
  });

  function choose(index: number, focus: boolean): void {
    active = index;
    onselect(index, focus);
  }

  function move(to: number): void {
    if (shown.length === 0) return;
    active = Math.max(0, Math.min(to, shown.length - 1));
    // Selection follows the cursor, so arrowing down the list scrolls the
    // editor to each one -- but focus stays here, or the next arrow key would
    // be typed into the document.
    onselect(active, false);
    list?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onKeyDown(event: KeyboardEvent): void {
    switch (event.key) {
      case "ArrowDown":
        move(active + 1);
        break;
      case "ArrowUp":
        move(active - 1);
        break;
      case "Home":
        move(0);
        break;
      case "End":
        move(shown.length - 1);
        break;
      case "Enter":
      case " ":
        choose(active, true);
        break;
      case "Escape":
        onescape();
        break;
      default:
        return;
    }
    event.preventDefault();
  }
</script>

<!-- `tabindex="-1"` for the collapsed case, where the listbox does not
     exist and the region itself is where F6 has to land. -->
<section class="panel" class:open data-pane tabindex="-1" aria-label="Diagnostics">
  <h2>
    <button type="button" onclick={ontoggle} aria-expanded={open} aria-controls="diagnostics-list">
      <span class="chevron" aria-hidden="true">{open ? "▾" : "▸"}</span>
      Diagnostics
      <span class="counts">
        {#each ["error", "warning", "information"] as const as severity}
          {#if counts[severity] > 0}
            <span class="count {severity}">
              <span aria-hidden="true">{GLYPH[severity]}</span>
              {counts[severity]}
              <span class="visually-hidden">{LABEL[severity]}</span>
            </span>
          {/if}
        {/each}
        {#if diagnostics.length === 0}<span class="clean">none</span>{/if}
      </span>
    </button>
  </h2>

  <!-- Announced on a delay, and separate from the visible counts so the
       reading is a settled sentence rather than three numbers. -->
  <p class="visually-hidden" aria-live="polite">{announced}</p>

  {#if open}
    <ul
      id="diagnostics-list"
      class="list"
      role="listbox"
      tabindex="0"
      data-pane-focus
      aria-label="Diagnostics"
      aria-activedescendant={shown.length > 0 ? `diagnostic-${active}` : undefined}
      bind:this={list}
      onkeydown={onKeyDown}
    >
      {#each shown as diagnostic, index (diagnostic.code + ":" + diagnostic.start + ":" + index)}
        <!--
          The keyboard handler is on the listbox, not on each option, which is
          what the ARIA pattern asks for: one tab stop, arrow keys within,
          `aria-activedescendant` naming the current row. Giving every option
          its own handler would mean making every option focusable, which is
          the five-hundred-tab-stops list this pattern exists to avoid.
        -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id="diagnostic-{index}"
          data-index={index}
          role="option"
          aria-selected={index === active}
          class:active={index === active}
          onclick={() => choose(index, false)}
          ondblclick={() => choose(index, true)}
        >
          <span class="glyph {diagnostic.severity}" aria-hidden="true">
            {GLYPH[diagnostic.severity]}
          </span>
          <span class="visually-hidden">{LABEL[diagnostic.severity]},</span>
          <span class="where">Line {diagnostic.line}</span>
          <code>{diagnostic.code}</code>
          <span class="message">{diagnostic.message}</span>
        </li>
      {/each}

      {#if diagnostics.length > shown.length}
        <li class="more" role="presentation">
          Showing {shown.length} of {diagnostics.length}.
        </li>
      {/if}
      {#if diagnostics.length === 0}
        <li class="more" role="presentation">Nothing to report.</li>
      {/if}
    </ul>
  {/if}
</section>

<style>
  .panel {
    flex: 0 0 auto;
    border-block-start: 1px solid var(--border);
    background: var(--surface-sunken);
    display: flex;
    flex-direction: column;
    min-block-size: 0;
  }

  .panel.open {
    /* Bounded, so a document full of errors cannot squeeze the editor out. */
    max-block-size: 30vh;
  }

  h2 {
    margin: 0;
    font-size: 0.8125rem;
    font-weight: 500;
    flex: 0 0 auto;
  }

  h2 button {
    inline-size: 100%;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding-block: 0.35rem;
    padding-inline: 0.6rem;
    background: none;
    border: none;
    color: var(--text-muted);
    font: inherit;
    text-align: start;
    cursor: pointer;
  }

  h2 button:hover {
    color: var(--text);
  }

  .chevron {
    inline-size: 0.75em;
  }

  .counts {
    display: flex;
    gap: 0.75rem;
    margin-inline-start: auto;
  }

  .count {
    font-variant-numeric: tabular-nums;
  }

  .clean {
    opacity: 0.7;
  }

  .list {
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
    flex: 1 1 auto;
    min-block-size: 0;
    font-size: 0.8125rem;
  }

  .list:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  li {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    padding-block: 0.25rem;
    padding-inline: 0.6rem;
    cursor: default;
  }

  li.active {
    background: color-mix(in srgb, var(--accent) 20%, transparent);
  }

  li:hover:not(.more) {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .where {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    flex: 0 0 auto;
  }

  code {
    font-family: var(--font-gutter);
    font-size: 0.85em;
    color: var(--text-muted);
    flex: 0 0 auto;
  }

  .message {
    /* The one part allowed to be long; the rest of the row is fixed width. */
    min-inline-size: 0;
  }

  .more {
    color: var(--text-muted);
    font-style: italic;
  }

  /* Shape carries the severity; colour only reinforces it (PRODUCT §10). */
  .glyph,
  .count {
    flex: 0 0 auto;
  }

  .glyph.error,
  .count.error {
    color: var(--severity-error);
  }

  .glyph.warning,
  .count.warning {
    color: var(--severity-warning);
  }

  .glyph.information,
  .count.information {
    color: var(--severity-information);
  }

  .visually-hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
