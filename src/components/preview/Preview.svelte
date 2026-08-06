<script lang="ts">
  /**
   * The reading pane (PRODUCT §7).
   *
   * ARCHITECTURE §10's shape exactly: a keyed each block over chunks, so only
   * the chapter whose `rev` moved is rebuilt. Typing in chapter forty must not
   * re-render chapter one, and that is a property of the key rather than of
   * anything this component does per update.
   *
   * `content-visibility` and first-paint overscan are P3.5. The keying is here
   * because retrofitting it means finding every place that assumed a flat
   * list, and there is no reason to write that list first.
   */

  import type { Chunk, PreviewNode } from "../../worker/protocol";
  import NodeView from "./NodeView.svelte";

  interface Props {
    chunks: Chunk[];
    /** Nodes per chunk, by index. `undefined` while a chapter is in flight. */
    previews: (PreviewNode[] | undefined)[];
    /** Clicking a verse moves the editor cursor there (PRODUCT §7). */
    onselect?: (start: number, end: number) => void;
  }

  let { chunks, previews, onselect }: Props = $props();

  /**
   * A stable identity per chapter.
   *
   * The chunk's number where it has one, and its index otherwise — the header
   * chunk has no number, and there is exactly one of it. Keying on index alone
   * would rebuild every chapter after an inserted `\c`; keying on number alone
   * cannot represent the header.
   */
  const keyOf = (chunk: Chunk, index: number) =>
    chunk.number === null ? `header:${index}` : `chapter:${chunk.number}`;
</script>

<div class="preview">
  {#each chunks as chunk, index (keyOf(chunk, index))}
    <section
      class="chapter"
      data-rev={chunk.rev}
      aria-label={chunk.number === null ? "Front matter" : `Chapter ${chunk.number}`}
    >
      {#if previews[index]}
        {#each previews[index] as node, at (at)}
          <NodeView {node} {onselect} />
        {/each}
      {:else}
        <!-- In flight. Deliberately not a spinner: a chapter arrives in a few
             milliseconds, and a spinner that appears and vanishes that fast
             reads as a flicker rather than as progress. -->
        <p class="pending" aria-hidden="true"></p>
      {/if}
    </section>
  {/each}

  {#if chunks.length === 0}
    <p class="empty">Nothing to preview yet.</p>
  {/if}
</div>

<style>
  .preview {
    block-size: 100%;
    overflow: auto;
    padding-block: 1rem;
    padding-inline: 1.5rem;
    /* Scripture, so the content font and the document's leading (UNICODE §7). */
    font-family: var(--font-content);
    line-height: var(--line-height);
  }

  .chapter {
    /* A measure, not the pane's width. Long lines are hard to read and this
       pane exists to be read. */
    max-inline-size: 38rem;
  }

  .pending {
    min-block-size: 1.5em;
    margin: 0;
  }

  .empty {
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  /* ------------------------------------------------------------ USFM ---
   *
   * Semantic classes from the marker, so `\q1`, `\li2` and every level the
   * specification numbers open-endedly can be styled without the renderer
   * enumerating them.
   */

  :global(.usfm-chapter) {
    font-size: 1.6rem;
    font-weight: 700;
    margin-block: 1.4rem 0.6rem;
    color: var(--text);
  }

  :global(.usfm-para) {
    margin-block: 0 0.7rem;
    /* Justified text needs hyphenation to avoid rivers, and hyphenation
       dictionaries do not exist for most of these scripts. */
    text-align: start;
  }

  /* A verse number is a reference mark, not part of the sentence. Raised and
     smaller, the way every printed Bible sets it. */
  :global(.usfm-verse) {
    font-size: 0.7em;
    font-weight: 600;
    vertical-align: super;
    color: var(--syntax-verse);
    margin-inline-end: 0.15em;
    /* A Latin number inside a right-to-left verse belongs where it was
       written, not dragged to the other end of the line (UNICODE §8). */
    unicode-bidi: isolate;
    direction: ltr;
  }

  /* Character styles. Only the ones P3.1 covers; the rest arrive with their
     own items rather than being guessed at here. */
  :global(.usfm-bd),
  :global(.usfm-bdit) {
    font-weight: 700;
  }

  :global(.usfm-it),
  :global(.usfm-bdit),
  :global(.usfm-em) {
    font-style: italic;
  }

  :global(.usfm-nd) {
    font-variant-caps: small-caps;
  }

  :global(.usfm-sc) {
    font-variant-caps: all-small-caps;
  }

  :global(.usfm-wj) {
    color: var(--severity-error);
  }

  :global(.usfm-qs) {
    font-style: italic;
    float: inline-end; /* lint-logical-ok: `float` has no logical shorthand */
  }

  /* Titles and headings, which are paragraphs in USFM's model. */
  :global(.usfm-mt1) {
    font-size: 1.5rem;
    font-weight: 700;
    text-align: center;
    margin-block: 1rem;
  }

  :global(.usfm-mt2),
  :global(.usfm-mt3) {
    font-size: 1.15rem;
    font-weight: 600;
    text-align: center;
  }

  :global(.usfm-s),
  :global(.usfm-s1),
  :global(.usfm-s2) {
    font-weight: 600;
    margin-block: 1.1rem 0.4rem;
  }

  :global(.usfm-r),
  :global(.usfm-sr) {
    font-style: italic;
    color: var(--text-muted);
    font-size: 0.9em;
  }

  /* Markup the parser could not interpret. Visibly not Scripture, because a
     placeholder that looked like text would be the preview quietly asserting
     that malformed markup is fine (ADR-003). */
  :global(.usfm-raw) {
    font-family: var(--font-gutter);
    font-size: 0.85em;
    padding-inline: 0.2em;
    border-radius: 2px;
    background: color-mix(in srgb, var(--severity-warning) 22%, transparent);
    unicode-bidi: isolate;
  }
</style>
