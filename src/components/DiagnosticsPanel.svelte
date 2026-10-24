<script lang="ts">
  /**
   * The diagnostics table, docked below the panes (PRODUCT §4).
   *
   * A listbox rather than a stack of buttons. Five hundred diagnostics as five
   * hundred buttons is five hundred tab stops between the editor and the
   * status bar, which is the kind of thing that technically passes an
   * accessibility audit and is unusable in practice (PRODUCT §10). One tab
   * stop, arrow keys within.
   *
   * The redesign turns the list into columns — severity, line, code, message,
   * and a way in — because a diagnostic is a record with four fields and
   * reading a hundred of them means reading down one column at a time. The
   * semantics did not change with the look: it is still one listbox, still
   * `aria-activedescendant`, still selection-follows-cursor.
   *
   * The filter is here rather than in the engine. Every diagnostic has already
   * been computed; hiding warnings is a question about the table, and sending
   * it to the worker would mean a round trip to answer something this side
   * already knows.
   */

  import Icon from "./Icon.svelte";
  import Segmented from "./Segmented.svelte";
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

  type Filter = "all" | "error" | "warning";
  let filter = $state<Filter>("all");

  const counts = $derived.by(() => {
    const counts = { error: 0, warning: 0, information: 0 };
    for (const diagnostic of diagnostics) counts[diagnostic.severity] += 1;
    return counts;
  });

  /**
   * The rows, each remembering where it came from.
   *
   * The index the editor is given has to be the index in the *unfiltered*
   * list, because that is the list it holds. Filtering and then handing back a
   * position in the filtered array would jump to a different diagnostic
   * whenever anything was hidden — and only then, which is the worst kind of
   * bug to find.
   */
  const rows = $derived(
    diagnostics
      .map((diagnostic, index) => ({ diagnostic, index }))
      .filter(({ diagnostic }) => filter === "all" || diagnostic.severity === filter)
      .slice(0, SHOWN),
  );

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
    if (active >= rows.length) active = Math.max(0, rows.length - 1);
  });

  /**
   * When the last parse landed, so the header can say how fresh this is.
   *
   * Tracked on the array's identity: the engine replaces it on every parse,
   * including one that found nothing, which is exactly the event being timed.
   */
  let checked = $state(Date.now());
  $effect(() => {
    diagnostics;
    checked = Date.now();
  });

  let now = $state(Date.now());
  $effect(() => {
    if (!open) return;
    // Five seconds, not one. The reading is "roughly how stale is this", and a
    // timer that fires every second all day to move a number nobody is
    // watching is a timer that should not exist.
    const timer = setInterval(() => (now = Date.now()), 5000);
    return () => clearInterval(timer);
  });

  const freshness = $derived.by(() => {
    const seconds = Math.max(0, Math.round((now - checked) / 1000));
    if (seconds < 5) return "just now";
    if (seconds < 60) return `${seconds}s ago`;
    return `${Math.round(seconds / 60)}m ago`;
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

  function choose(at: number, focus: boolean): void {
    active = at;
    const row = rows[at];
    if (row) onselect(row.index, focus);
  }

  function move(to: number): void {
    if (rows.length === 0) return;
    active = Math.max(0, Math.min(to, rows.length - 1));
    // Selection follows the cursor, so arrowing down the list scrolls the
    // editor to each one -- but focus stays here, or the next arrow key would
    // be typed into the document.
    const row = rows[active];
    if (row) onselect(row.index, false);
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
        move(rows.length - 1);
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

  const filters = [
    { value: "all" as const, label: "All" },
    { value: "error" as const, label: "Errors" },
    { value: "warning" as const, label: "Warnings" },
  ];
</script>

<!-- `tabindex="-1"` for the collapsed case, where the listbox does not
     exist and the region itself is where F6 has to land. -->
<section class="panel" class:open data-pane tabindex="-1" aria-label="Diagnostics">
  <div class="head">
    <h2>
      <button type="button" onclick={ontoggle} aria-expanded={open} aria-controls="diagnostics-list">
        <span class="chevron" aria-hidden="true">{open ? "▾" : "▸"}</span>
        Diagnostics
      </button>
    </h2>

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

    {#if open}
      <Segmented
        name="diagnostics-filter"
        label="Filter diagnostics"
        options={filters}
        value={filter}
        onchange={(value) => (filter = value)}
        compact
      />
    {/if}

    <span class="fresh">Live · re-checked {freshness}</span>
  </div>

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
      aria-activedescendant={rows.length > 0 ? `diagnostic-${active}` : undefined}
      bind:this={list}
      onkeydown={onKeyDown}
    >
      {#each rows as row, at (row.diagnostic.code + ":" + row.diagnostic.start + ":" + row.index)}
        <!--
          The keyboard handler is on the listbox, not on each option, which is
          what the ARIA pattern asks for: one tab stop, arrow keys within,
          `aria-activedescendant` naming the current row. Giving every option
          its own handler would mean making every option focusable, which is
          the five-hundred-tab-stops list this pattern exists to avoid.
        -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id="diagnostic-{at}"
          data-index={at}
          role="option"
          aria-selected={at === active}
          class:active={at === active}
          onclick={() => choose(at, false)}
          ondblclick={() => choose(at, true)}
        >
          <span class="glyph {row.diagnostic.severity}" aria-hidden="true">
            {GLYPH[row.diagnostic.severity]}
          </span>
          <span class="visually-hidden">{LABEL[row.diagnostic.severity]},</span>
          <span class="where">L{row.diagnostic.line}</span>
          <code>{row.diagnostic.code}</code>
          <span class="message">{row.diagnostic.message}</span>
          <span class="jump" aria-hidden="true">Jump <Icon name="jump" /></span>
        </li>
      {/each}

      {#if diagnostics.length > rows.length}
        <li class="more" role="presentation">
          Showing {rows.length} of {diagnostics.length}.
        </li>
      {/if}
      {#if rows.length === 0}
        <li class="more" role="presentation">
          {diagnostics.length === 0 ? "Nothing to report." : "Nothing matches this filter."}
        </li>
      {/if}
    </ul>
  {/if}
</section>

<style>
  .panel {
    flex: 0 0 auto;
    border-block-start: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    min-block-size: 0;
  }

  .panel.open {
    /* Bounded, so a document full of errors cannot squeeze the editor out. */
    max-block-size: 30vh;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding-inline: 1rem;
    block-size: 2.125rem;
    border-block-end: 1px solid var(--line2);
    flex: 0 0 auto;
    font-size: 0.75rem;
  }

  h2 {
    margin: 0;
    font-size: inherit;
    font-weight: 500;
  }

  h2 button {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0;
    background: none;
    border: none;
    color: var(--fg);
    font: inherit;
    font-weight: 500;
    cursor: pointer;
  }

  .chevron {
    inline-size: 0.75em;
    color: var(--fg3);
  }

  .counts {
    display: flex;
    gap: 0.75rem;
    color: var(--fg2);
  }

  .count {
    font-variant-numeric: tabular-nums;
  }

  .clean {
    color: var(--fg3);
  }

  .fresh {
    margin-inline-start: auto;
    color: var(--fg3);
    white-space: nowrap;
  }

  .list {
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
    flex: 1 1 auto;
    min-block-size: 0;
    font-size: 0.78125rem;
  }

  .list:focus-visible {
    outline: 2px solid var(--acc);
    outline-offset: -2px;
  }

  li {
    display: grid;
    grid-template-columns: 1.25rem 3.5rem 6.5rem minmax(0, 1fr) auto;
    align-items: center;
    column-gap: 0.75rem;
    block-size: 1.875rem;
    padding-inline: 1rem;
    border-block-end: 1px solid var(--line2);
    cursor: default;
  }

  li.more {
    display: block;
    color: var(--fg3);
    font-style: italic;
    line-height: 1.875rem;
  }

  li.active {
    background: var(--hl);
  }

  li:hover:not(.more) {
    background: var(--hl);
  }

  .where {
    color: var(--fg2);
    font-family: var(--font-mono);
    font-size: 0.71875rem;
    font-variant-numeric: tabular-nums;
  }

  code {
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--fg3);
  }

  .message {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The way in. Only on the row being read, because a column of "Jump" on
     every row is a column of noise. */
  .jump {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--acc-text);
    font-weight: 500;
    font-size: 0.75rem;
    visibility: hidden;
  }

  li.active .jump,
  li:hover .jump {
    visibility: visible;
  }

  /* Shape carries the severity; colour only reinforces it (PRODUCT §10). */
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
</style>
