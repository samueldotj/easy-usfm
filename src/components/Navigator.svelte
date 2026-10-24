<script lang="ts">
  /**
   * Where you are in the book, and where you can go next.
   *
   * A grid of chapter numbers and the sections inside the one being read. Both
   * are views of the parse, so both are current without anything being told to
   * refresh — and the dot on a chapter is what makes the grid worth its column:
   * twenty-one numbers is a table of contents, twenty-one numbers with one dot
   * is a place to go next.
   *
   * # One tab stop, arrow keys within
   *
   * A book has up to a hundred and fifty chapters. A hundred and fifty buttons
   * is a hundred and fifty tab stops between the top of the window and the
   * editor, which technically passes an audit and is unusable in practice
   * (PRODUCT §10). So the grid is a roving tabindex: one stop, and the arrows
   * walk it — including up and down, because it is a grid and a reader who
   * presses Down expects to move a row rather than seven chapters.
   *
   * # Study shrinks it rather than hiding it
   *
   * The rail keeps the numbers and the dots and loses the outline, which the
   * breadcrumb in that layout carries instead.
   */

  import { layout } from "../lib/layout.svelte";
  import { verseRange, type ChapterEntry, type Section } from "../lib/outline";

  interface Props {
    chapters: ChapterEntry[];
    sections: Section[];
    /** Index into `chapters` of the one the caret is in. */
    activeChapter: number;
    /** Index into `sections`, or -1 above the first heading. */
    activeSection: number;
    /** How many verses the current chapter has, for the footer. */
    verses: number;
    /** Move the caret to an offset in the source. */
    ongo: (offset: number) => void;
  }

  let { chapters, sections, activeChapter, activeSection, verses, ongo }: Props = $props();

  /** Columns in the grid. The mock-up's seven, which fits 1–31 in five rows. */
  const COLUMNS = 7;

  let grid = $state<HTMLElement>();

  /** Which cell the arrows are on. Follows the caret until the user moves it. */
  let roving = $state(0);
  $effect(() => {
    roving = Math.max(0, activeChapter);
  });

  const current = $derived(chapters[activeChapter]);

  function focusCell(index: number): void {
    const at = Math.max(0, Math.min(index, chapters.length - 1));
    roving = at;
    grid?.querySelector<HTMLElement>(`[data-cell="${at}"]`)?.focus();
  }

  function onGridKey(event: KeyboardEvent): void {
    const step =
      event.key === "ArrowRight"
        ? 1
        : event.key === "ArrowLeft"
          ? -1
          : event.key === "ArrowDown"
            ? COLUMNS
            : event.key === "ArrowUp"
              ? -COLUMNS
              : event.key === "Home"
                ? -roving
                : event.key === "End"
                  ? chapters.length - 1 - roving
                  : null;

    if (step === null) return;
    event.preventDefault();
    focusCell(roving + step);
  }

  /** The label a screen reader hears, since the cell itself is just a number. */
  function cellLabel(entry: ChapterEntry): string {
    const name = entry.number === null ? "Front matter" : `Chapter ${entry.number}`;
    if (entry.severity === null) return name;
    return `${name}, has ${entry.severity === "error" ? "an error" : "a warning"}`;
  }
</script>

{#if layout.shell === "workbench"}
  <nav class="panel" aria-label="Navigator">
    <div class="head">
      <h2>Chapters</h2>
      <span class="count">{chapters.filter((entry) => entry.number !== null).length}</span>
    </div>

    <div class="grid" bind:this={grid}>
      {#each chapters as entry, index (entry.index)}
        <button
          type="button"
          class="cell"
          class:on={index === activeChapter}
          class:front={entry.number === null}
          data-cell={index}
          tabindex={index === roving ? 0 : -1}
          aria-current={index === activeChapter ? "true" : undefined}
          aria-label={cellLabel(entry)}
          title={cellLabel(entry)}
          onclick={() => ongo(entry.start)}
          onfocus={() => (roving = index)}
          onkeydown={onGridKey}
        >
          {entry.label}
          {#if entry.severity !== null}
            <span class="mark {entry.severity}" aria-hidden="true"></span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="head second">
      <h2>
        Outline{#if current && current.number !== null}<span class="thin"> · Ch {current.number}</span>{/if}
      </h2>
      <span class="count">
        {sections.length}
        {sections.length === 1 ? "section" : "sections"}
      </span>
    </div>

    <ul class="outline">
      {#each sections as section, index (section.start)}
        <li>
          <button
            type="button"
            class="section"
            class:on={index === activeSection}
            aria-current={index === activeSection ? "true" : undefined}
            onclick={() => ongo(section.start)}
          >
            <span class="bar" aria-hidden="true"></span>
            <span class="title">{section.title || "Untitled section"}</span>
            <span class="verses">{verseRange(section)}</span>
          </button>
        </li>
      {/each}
      {#if sections.length === 0}
        <li class="none">No headings in this chapter.</li>
      {/if}
    </ul>

    <div class="foot">
      <span>
        {#if current && current.number !== null}Ch {current.number} · {/if}{verses}
        {verses === 1 ? "verse" : "verses"}
      </span>
      <button type="button" class="collapse" onclick={() => layout.toggleNavigator()}>
        Collapse ‹
      </button>
    </div>
  </nav>
{:else}
  <nav class="rail" aria-label="Chapters">
    <div class="rail-head">Ch</div>
    <div class="rail-list" bind:this={grid}>
      {#each chapters as entry, index (entry.index)}
        <button
          type="button"
          class="cell rail-cell"
          class:on={index === activeChapter}
          data-cell={index}
          tabindex={index === roving ? 0 : -1}
          aria-current={index === activeChapter ? "true" : undefined}
          aria-label={cellLabel(entry)}
          title={cellLabel(entry)}
          onclick={() => ongo(entry.start)}
          onfocus={() => (roving = index)}
          onkeydown={onGridKey}
        >
          {entry.number === null ? "·" : entry.number}
          {#if entry.severity !== null}
            <span class="mark {entry.severity}" aria-hidden="true"></span>
          {/if}
        </button>
      {/each}
    </div>
  </nav>
{/if}

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    overflow: hidden;
    background: var(--bg2);
    border-inline-end: 1px solid var(--line);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-block: 0.875rem 0.625rem;
    padding-inline: 0.875rem;
    flex: 0 0 auto;
  }

  .head.second {
    padding-block-start: 1.25rem;
  }

  h2 {
    margin: 0;
    font-size: 0.65625rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg3);
  }

  .thin {
    letter-spacing: 0;
  }

  .count {
    font-size: 0.6875rem;
    color: var(--fg3);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 3px;
    padding-inline: 0.75rem;
    flex: 0 0 auto;
  }

  .cell {
    position: relative;
    aspect-ratio: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius-sm);
    background: var(--bg3);
    color: var(--fg2);
    font: inherit;
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }

  /* The front matter is not a chapter and does not have a number; it takes the
     whole first row rather than pretending to be chapter zero. */
  .cell.front {
    grid-column: 1 / -1;
    aspect-ratio: auto;
    padding-block: 0.2rem;
    font-size: 0.6875rem;
    letter-spacing: 0.04em;
  }

  .cell:hover {
    background: var(--hl);
    color: var(--fg);
  }

  .cell.on {
    background: var(--acc);
    color: var(--acc-ink);
    font-weight: 600;
  }

  /* Shape as well as colour: an error is a filled dot and a warning a hollow
     ring, so the grid still says which is which in monochrome (PRODUCT §10). */
  .mark {
    position: absolute;
    inset-block-start: 3px;
    inset-inline-end: 3px;
    inline-size: 5px;
    block-size: 5px;
    border-radius: 50%;
  }

  .mark.error {
    background: var(--err);
  }

  .mark.warning,
  .mark.information {
    border: 1.5px solid var(--acc);
  }

  .cell.on .mark.warning,
  .cell.on .mark.information {
    border-color: var(--acc-ink);
  }

  .outline {
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
    flex: 1 1 auto;
    min-block-size: 0;
  }

  .section {
    inline-size: 100%;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding-block: 0.375rem;
    padding-inline: 0.6875rem 0.875rem;
    border: none;
    background: none;
    color: var(--fg2);
    font: inherit;
    font-size: 0.78125rem;
    text-align: start;
    cursor: pointer;
  }

  .section:hover {
    background: var(--hl);
    color: var(--fg);
  }

  .section.on {
    color: var(--fg);
    font-weight: 500;
  }

  .bar {
    inline-size: 3px;
    block-size: 0.875rem;
    border-radius: 2px;
    background: transparent;
    flex: 0 0 auto;
  }

  .section.on .bar {
    background: var(--acc);
  }

  .title {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .verses {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--fg3);
  }

  .none {
    padding-block: 0.375rem;
    padding-inline: 0.875rem;
    font-size: 0.75rem;
    color: var(--fg3);
    font-style: italic;
  }

  .foot {
    margin-block-start: auto;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
    padding-block: 0.75rem;
    padding-inline: 0.875rem;
    border-block-start: 1px solid var(--line);
    font-size: 0.71875rem;
    color: var(--fg3);
    flex: 0 0 auto;
  }

  .collapse {
    border: none;
    background: none;
    color: var(--acc-text);
    font: inherit;
    font-size: inherit;
    cursor: pointer;
    padding: 0;
  }

  .collapse:hover {
    text-decoration: underline;
  }

  /* ------------------------------------------------------------ rail ---- */

  .rail {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg2);
    border-inline-end: 1px solid var(--line);
  }

  .rail-head {
    font-size: 0.59375rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg3);
    text-align: center;
    padding-block: 0.625rem 0.375rem;
    flex: 0 0 auto;
  }

  .rail-list {
    overflow: auto;
    flex: 1 1 auto;
    min-block-size: 0;
    display: flex;
    flex-direction: column;
  }

  .rail-cell {
    aspect-ratio: auto;
    block-size: 2.125rem;
    margin-block-end: 2px;
    margin-inline: 0.5rem;
    background: transparent;
    flex: 0 0 auto;
  }

  .rail-cell:hover {
    background: var(--hl);
  }

  .rail-cell.on {
    background: var(--acc);
  }
</style>
