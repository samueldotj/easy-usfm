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
  import { collapse, examplesFor, grouped } from "../lib/markerGroups";
  import NodeView from "./preview/NodeView.svelte";
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

  /**
   * Grouped by what the markers do together, not by class.
   *
   * A table is five markers that mean nothing apart, and an alphabetical list
   * by class puts `	c` a long way from `	r`. Each group leads with the
   * markers in use *together*, which is the part worth copying and the part a
   * per-marker list structurally cannot show.
   */
  const groups = $derived(grouped(shown));

  /** Whether a search is narrowing the list, which changes what to show. */
  const searching = $derived(query.trim() !== "");
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

    {#each groups as group (group.id)}
      <section>
        <h3>{group.title}</h3>
        <p class="blurb">{group.blurb}</p>

        <!--
          The markers in use together. Hidden while searching, because a
          combined example for a group the reader has filtered down to one
          marker is showing them four markers they did not ask about.
        -->
        {#if group.example && !searching}
          <pre class="combined">{group.example}</pre>
        {/if}

        <!--
          One entry per family, not per marker. `\h`, `\h1`, `\h2` and `\h3`
          are one thing with four spellings, and four entries repeating the
          same sentence is the wall of text this page exists to avoid.
        -->
        {#each collapse(group.markers) as entry (entry.stem)}
          {@const help = entry.help}
          {@const source = examplesFor(entry)}
          <article class:deprecated={entry.anyDeprecated}>
            <h4>
              <code>{entry.label}</code>
              {#if entry.levels.length > 1}
                <span class="tag">{entry.levels.length} levels</span>
              {/if}
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

            <pre>{source}</pre>

            <!--
              What the example produces. A reference that says `\q1` is a
              poetry line tells you less than one that shows the indent, and
              the levels of a family only differ in ways you can see.
            -->
            {#await engine.previewSnippet(source) then nodes}
              {#if nodes.length > 0}
                <div class="rendered preview" aria-label="Rendered result">
                  {#each nodes as node, at (at)}
                    <NodeView {node} />
                  {/each}
                </div>
              {/if}
            {/await}

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

  .blurb {
    margin-block: 0 0.5rem;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  /* What an example produces. Given the preview's own class so the Scripture
     styles apply, and boxed so it reads as output rather than as more page. */
  .rendered {
    margin-block: 0.35rem 0.2rem;
    padding-block: 0.5rem;
    padding-inline: 0.7rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    font-size: 0.9rem;
  }

  /* The group's markers in use together — the part worth copying. Set apart
     from the per-marker snippets so it reads as the worked example rather than
     as the first entry's syntax. */
  .combined {
    margin-block-end: 0.6rem;
    padding-block: 0.5rem;
    padding-inline: 0.7rem;
    border-inline-start: 3px solid var(--accent);
    white-space: pre;
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
