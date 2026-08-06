<script lang="ts">
  /**
   * The USFM marker reference.
   *
   * Every marker the parser knows, generated from the same table it parses
   * with — so the page cannot come to disagree with the editor about what is
   * accepted. A reference written separately drifts within a release, and the
   * reader has no way to tell which entry is the stale one.
   *
   * A dialog rather than a route: this application has no router (ARCHITECTURE
   * §1), and the reference is something you consult while editing and then
   * dismiss, not somewhere you navigate to and come back from.
   */

  import { engine } from "../lib/engine.svelte";
  import { helpTable, matches, type MarkerHelp } from "../lib/markerHelp";

  let dialog: HTMLDialogElement | undefined = $state();
  let query = $state("");
  let table = $state<MarkerHelp[]>([]);
  let failed = $state(false);

  export async function open(): Promise<void> {
    // Loaded on first open rather than at startup: 335 rows cost nothing to
    // fetch and nothing to keep, but they are of no use to someone who never
    // opens the reference.
    if (table.length === 0 && !failed) {
      try {
        table = helpTable(await engine.markerTable());
      } catch {
        failed = true;
      }
    }
    dialog?.showModal();
  }

  const shown = $derived(table.filter((help) => matches(help, query)));

  /** Grouped by class, in the order a reader meets them. */
  const ORDER = ["paragraph", "character", "note", "milestone", "unclassified"] as const;

  const groups = $derived(
    ORDER.map((name) => ({ name, markers: shown.filter((help) => help.class === name) })).filter(
      (group) => group.markers.length > 0,
    ),
  );

  const LABELS: Record<string, string> = {
    paragraph: "Paragraph markers — begin a line and own the text after it",
    character: "Character markers — wrap a span of text inside a paragraph",
    note: "Notes — footnotes and cross-references",
    milestone: "Milestones — mark a position rather than a span",
    unclassified: "Other markers",
  };
</script>

<dialog bind:this={dialog} aria-label="USFM marker reference">
  <header>
    <h2>USFM markers</h2>
    <input
      type="search"
      placeholder="Search markers, or what they do…"
      aria-label="Search markers"
      bind:value={query}
    />
    <button type="button" onclick={() => dialog?.close()} aria-label="Close">Close</button>
  </header>

  {#if failed}
    <p class="empty">The marker table could not be loaded.</p>
  {:else if table.length === 0}
    <p class="empty">Loading…</p>
  {:else if shown.length === 0}
    <p class="empty">No marker matches “{query}”.</p>
  {:else}
    <p class="count">
      {shown.length} of {table.length} markers
    </p>

    {#each groups as group (group.name)}
      <section>
        <h3>{LABELS[group.name] ?? group.name}</h3>

        {#each group.markers as help (help.marker)}
          <article class:deprecated={help.deprecated_in !== null}>
            <h4>
              <code>\{help.marker}</code>
              {#if help.deprecated_in}
                <span class="tag warn">deprecated in {help.deprecated_in}</span>
                {#if help.replacement}
                  <span class="tag">use <code>\{help.replacement}</code></span>
                {/if}
              {/if}
              {#if help.since}
                <span class="tag">since {help.since}</span>
              {/if}
              {#if !help.publishable}
                <span class="tag">not printed</span>
              {/if}
            </h4>

            {#if help.description}
              <p>{help.description}</p>
            {:else}
              <!--
                No sentence rather than an invented one. A reference that
                confidently describes a marker wrongly is worse than one that
                says nothing, because the reader cannot tell which is which.
              -->
              <p class="unknown">
                No description yet — the syntax below comes from the
                specification's own stylesheet.
              </p>
            {/if}

            <pre>{help.example}</pre>

            {#if help.attributes.length > 0}
              <p class="detail">
                Attributes: {#each help.attributes as attribute, index (attribute)}<code
                    >{attribute}</code
                  >{#if help.default_attr === attribute}<span class="tag">default</span
                    >{/if}{#if index < help.attributes.length - 1},
                {/if}{/each}
              </p>
            {/if}
          </article>
        {/each}
      </section>
    {/each}
  {/if}
</dialog>

<style>
  dialog {
    inline-size: min(52rem, 92vw);
    block-size: min(80vh, 50rem);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 45%);
  }

  header {
    position: sticky;
    inset-block-start: 0;
    display: flex;
    gap: 0.6rem;
    align-items: center;
    padding-block: 0.7rem;
    padding-inline: 1rem;
    background: var(--surface);
    border-block-end: 1px solid var(--border);
  }

  h2 {
    margin: 0;
    font-size: 1.05rem;
    white-space: nowrap;
  }

  input {
    flex: 1 1 auto;
    padding-block: 0.25rem;
    padding-inline: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-sunken);
    color: var(--text);
    font: inherit;
  }

  header button {
    padding-block: 0.25rem;
    padding-inline: 0.7rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-sunken);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .count,
  .empty {
    margin: 0;
    padding-block: 0.6rem;
    padding-inline: 1rem;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  section {
    padding-inline: 1rem;
  }

  h3 {
    margin-block: 1rem 0.5rem;
    font-size: 0.9rem;
    color: var(--text-muted);
    font-weight: 600;
  }

  article {
    padding-block: 0.6rem;
    border-block-start: 1px solid var(--border);
  }

  article.deprecated code {
    opacity: 0.75;
  }

  h4 {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    align-items: center;
    margin: 0 0 0.3rem;
    font-size: 0.95rem;
  }

  p {
    margin: 0 0 0.4rem;
    font-size: 0.9rem;
    line-height: 1.5;
  }

  .unknown,
  .detail {
    color: var(--text-muted);
    font-size: 0.82rem;
  }

  pre {
    margin: 0;
    padding-block: 0.35rem;
    padding-inline: 0.6rem;
    border-radius: 4px;
    background: var(--surface-sunken);
    font-family: var(--font-gutter), monospace;
    font-size: 0.85rem;
    overflow-x: auto;
    /* A marker's syntax is left-to-right whatever the interface language. */
    unicode-bidi: isolate;
  }

  code {
    font-family: var(--font-gutter), monospace;
  }

  .tag {
    padding-block: 0.05rem;
    padding-inline: 0.35rem;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 400;
  }

  .tag.warn {
    border-color: #b45309;
    color: #b45309;
  }
</style>
