<script lang="ts">
  /**
   * What the caret is inside, and what that marker means.
   *
   * The reference page (F1) answers "what markers are there"; this answers
   * "what am I in", which is the question someone actually has while typing.
   * Same table, same sentences — `markerHelp` generates both, so the two can
   * never say different things about `\q1`.
   *
   * The table costs a WASM instantiation, so it is fetched the first time this
   * tab is opened and never again.
   */

  import { markerTable } from "../lib/markerTable.svelte";

  interface Props {
    /** The marker the caret is in, without its backslash. */
    marker: string | null;
    /** Opens the full reference page. */
    onreference: () => void;
  }

  let { marker, onreference }: Props = $props();

  $effect(() => {
    void markerTable.load();
  });

  const help = $derived(marker === null ? undefined : markerTable.find(marker));

  const CLASSES: Record<string, string> = {
    character: "Character marker — wraps a span of text inside a paragraph.",
    paragraph: "Paragraph marker — begins a line and owns the text after it.",
    note: "Note marker — opens a footnote or cross-reference.",
    milestone: "Milestone — marks a position, and pairs with a partner.",
    unclassified: "The marker table does not classify this one.",
  };
</script>

<div class="panel">
  {#if marker === null}
    <p class="empty">The caret is not inside a marker.</p>
  {:else}
    <div class="head">
      <code class="marker">\{marker}</code>
      <!-- The family, when it says something the marker has not already said.
           `\xt` belongs to the family `xt`, and printing both reads as a
           stutter. -->
      {#if help?.family && help.family !== marker}
        <span class="name">the <code>\{help.family}</code> family</span>
      {/if}
    </div>

    {#if help}
      {#if help.description}
        <p class="description">{help.description}</p>
      {/if}
      <p class="class">{CLASSES[help.class] ?? CLASSES["unclassified"]}</p>

      {#if help.deprecated_in}
        <p class="deprecated">
          Deprecated in USFM {help.deprecated_in}{#if help.replacement}. Use
            <code>\{help.replacement}</code> instead{/if}.
        </p>
      {/if}

      {#if help.attributes.length > 0}
        <div class="block">
          <h3>Attributes</h3>
          <p class="attributes">
            {#each help.attributes as attribute, index (attribute)}<code
                class:default={attribute === help.default_attr}>{attribute}</code
              >{#if index < help.attributes.length - 1}<span class="sep">·</span>{/if}{/each}
          </p>
        </div>
      {/if}

      <div class="block">
        <h3>Example</h3>
        <pre>{help.example}</pre>
      </div>

      <div class="tags">
        {#if help.since}<span class="tag">Since {help.since}</span>{/if}
        {#if !help.publishable}<span class="tag">Not published</span>{/if}
        <button type="button" class="ghost" onclick={onreference}>All markers (F1)</button>
      </div>
    {:else if markerTable.loading}
      <p class="empty">Loading the marker table…</p>
    {:else}
      <p class="empty">
        The marker table has no entry for <code>\{marker}</code>. It may be a private extension, or
        a typo.
      </p>
    {/if}
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding-block: 0.75rem;
    padding-inline: 0.875rem;
    overflow: auto;
    min-block-size: 0;
    font-size: 0.78125rem;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .marker {
    font-family: var(--font-mono);
    font-size: 1.0625rem;
    font-weight: 600;
    color: var(--mk);
  }

  .name {
    color: var(--fg2);
  }

  .description,
  .class,
  .deprecated {
    margin: 0;
    text-wrap: pretty;
  }

  .class {
    color: var(--fg3);
    font-size: 0.71875rem;
  }

  .deprecated {
    padding-block: 0.375rem;
    padding-inline: 0.5rem;
    border-inline-start: 2px solid var(--err);
    background: var(--hl);
    color: var(--fg2);
    font-size: 0.71875rem;
  }

  .block h3 {
    margin: 0 0 0.25rem;
    font-size: 0.6875rem;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg3);
  }

  pre {
    margin: 0;
    padding-block: 0.5rem;
    padding-inline: 0.625rem;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-family: var(--font-mono);
    font-size: 0.71875rem;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .attributes {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.375rem;
    align-items: baseline;
  }

  code {
    font-family: var(--font-mono);
  }

  .attributes code.default {
    color: var(--acc-text);
    font-weight: 600;
  }

  .sep {
    color: var(--fg3);
  }

  .tags {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    flex-wrap: wrap;
  }

  .tag {
    padding-block: 0.125rem;
    padding-inline: 0.4375rem;
    background: var(--bg3);
    border-radius: var(--radius-sm);
    color: var(--fg2);
    font-size: 0.6875rem;
  }

  .ghost {
    margin-inline-start: auto;
    border: none;
    background: none;
    color: var(--acc-text);
    font: inherit;
    font-size: 0.71875rem;
    font-weight: 500;
    cursor: pointer;
  }

  .ghost:hover {
    text-decoration: underline;
  }

  .empty {
    margin: 0;
    color: var(--fg3);
    font-size: 0.75rem;
  }
</style>
