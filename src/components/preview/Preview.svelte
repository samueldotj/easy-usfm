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
    /** A link the user chose to follow, opened outside the webview. */
    onfollow?: (href: string) => void;
    /** A scripture reference in a link, resolved rather than navigated. */
    onreference?: (reference: string) => void;
    /** Asks the engine to render a chapter. Called for what is about to show. */
    onneed?: (chunk: number) => void;
    /** The preview was scrolled. The editor follows (P3.6). */
    onscroll?: () => void;
    /** Asks the shell for a figure's image, when images are on (SECURITY §3). */
    onfigure?: (path: string) => void;
  }

  let {
    chunks,
    previews,
    onselect,
    onfollow,
    onreference,
    onneed,
    onscroll,
    onfigure,
  }: Props = $props();

  /** The scrolling element, for the scroll sync to measure and move. */
  export function container(): HTMLDivElement | undefined {
    return host;
  }

  /**
   * Which chapters are worth rendering now (ARCHITECTURE 10).
   *
   * "Chapters intersecting the viewport plus one screen of overscan render
   * immediately; the rest parses in the background." An observer answers the
   * first half exactly, and answers it again on every scroll without this
   * component computing geometry.
   *
   * `rootMargin: 100%` is the overscan: one screen either side, so a chapter
   * is asked for before it can be seen rather than as it appears.
   */
  let host = $state<HTMLDivElement>();

  $effect(() => {
    const root = host;
    if (!root || typeof IntersectionObserver === "undefined") return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          const index = Number((entry.target as HTMLElement).dataset.chunk);
          if (Number.isInteger(index)) onneed?.(index);
        }
      },
      { root, rootMargin: "100%" },
    );

    for (const section of root.querySelectorAll("[data-chunk]")) observer.observe(section);
    return () => observer.disconnect();
  });

  /**
   * The rest, once the browser has nothing better to do.
   *
   * Without this a chapter is rendered only when it nears the viewport, so
   * dragging the scrollbar to the end of a long book lands on an empty page
   * for a round trip. Filling in from idle time means the document is whole by
   * the time anyone gets there, without competing with first paint for it.
   */
  $effect(() => {
    const missing = chunks
      .map((_, index) => index)
      .filter((index) => previews[index] === undefined);
    if (missing.length === 0) return;

    // `requestIdleCallback` where it exists; Safari still does not have it,
    // and a timeout is a fair approximation of "not now".
    const schedule = globalThis.requestIdleCallback ?? ((fn: () => void) => setTimeout(fn, 200));
    const handle = schedule(() => {
      // One per idle period. The point is to be invisible, and asking for
      // fifty chapters in one callback is a frame nobody gets back.
      const next = missing[0];
      if (next !== undefined) onneed?.(next);
    });

    return () => {
      const cancel = globalThis.cancelIdleCallback ?? clearTimeout;
      cancel(handle as number);
    };
  });

  /**
   * Milestones with no partner, per chapter.
   *
   * PRODUCT §7: an unpaired milestone warns and renders as a chip "rather than
   * swallowing the rest of the document". Deciding that needs the whole
   * chapter, not one node — a `\qt-s` is only unpaired once you have looked at
   * everything after it — so it is computed here and handed down.
   *
   * Scoped to the chunk, which is what the parse is scoped to. A milestone
   * legally spans chapters, so one crossing a boundary reads as unpaired to
   * both halves; that is the same limitation the chunked parse has everywhere
   * and it errs towards showing the reader something rather than hiding it.
   */
  function unpairedIn(nodes: readonly PreviewNode[]): ReadonlySet<PreviewNode> {
    const open = new Map<string, PreviewNode[]>();
    const unpaired = new Set<PreviewNode>();

    const walk = (list: readonly PreviewNode[]) => {
      for (const node of list) {
        const marker = node.marker;
        if (node.kind === "ms" && marker) {
          const base = marker.replace(/-[se]$/, "");
          if (marker.endsWith("-s")) {
            const pending = open.get(base) ?? [];
            pending.push(node);
            open.set(base, pending);
          } else if (marker.endsWith("-e")) {
            const pending = open.get(base);
            // An end with nothing open is as unpaired as a start with nothing
            // closing it, and just as worth showing.
            if (pending && pending.length > 0) pending.pop();
            else unpaired.add(node);
          }
        }
        walk(node.children);
      }
    };

    walk(nodes);
    for (const pending of open.values()) for (const node of pending) unpaired.add(node);
    return unpaired;
  }

  /**
   * Every note in a chapter, in document order.
   *
   * Collected for print (PRODUCT 8): per-page footnotes need `float: footnote`,
   * which no browser has, so notes go to the end of the chapter or the end of
   * the document instead. Both collections are rendered and the stylesheet
   * prints one, because which of them to show is a setting and re-rendering a
   * book to change a radio button would be absurd.
   *
   * Nested notes are not descended into: a note inside a note is part of that
   * note's text, not a second entry in the list.
   */
  function notesIn(nodes: readonly PreviewNode[]): PreviewNode[] {
    const found: PreviewNode[] = [];

    const walk = (list: readonly PreviewNode[]) => {
      for (const node of list) {
        if (node.kind === "note") found.push(node);
        else walk(node.children);
      }
    };

    walk(nodes);
    return found;
  }

  /**
   * Whether a note is a cross-reference rather than a footnote.
   *
   * The one distinction print cares about: PRODUCT 8 leaves cross-references
   * out by default, because they are apparatus rather than Scripture.
   */
  const isReference = (note: PreviewNode) => note.marker === "x" || note.marker === "ex";

  /** Every note in the document, for the end-of-document collection. */
  const allNotes = $derived.by(() =>
    previews.flatMap((nodes) => (nodes ? notesIn(nodes) : [])),
  );

  /**
   * A stable identity per chapter.
   *
   * The chunk's number where it has one, and its index otherwise — the header
   * chunk has no number, and there is exactly one of it. Keying on index alone
   * would rebuild every chapter after an inserted `\c`; keying on number alone
   * cannot represent the header.
   *
   * The occurrence count is what makes it *unique*, which the number alone is
   * not. Nothing stops a document from carrying `\c 1` twice — a duplicated
   * chapter, a mis-numbered one, or the ordinary half-second between typing
   * `\c ` and typing the digit that tells it apart. A duplicate key throws out
   * of the each block, and everything from the collision onwards renders as
   * nothing at all: the preview goes blank for what is only a typo.
   */
  const keys = $derived.by(() => {
    const seen = new Map<number, number>();
    return chunks.map((chunk, index) => {
      if (chunk.number === null) return `header:${index}`;
      const nth = (seen.get(chunk.number) ?? 0) + 1;
      seen.set(chunk.number, nth);
      // Only the repeats are suffixed, so the common case keeps the plain key
      // and a chapter is not rebuilt because a later duplicate appeared.
      return nth === 1 ? `chapter:${chunk.number}` : `chapter:${chunk.number}#${nth}`;
    });
  });
</script>

<div class="preview" bind:this={host} onscroll={() => onscroll?.()}>
  {#each chunks as chunk, index (keys[index])}
    <section
      class="chapter"
      data-chunk={index}
      data-rev={chunk.rev}
      aria-label={chunk.number === null ? "Front matter" : `Chapter ${chunk.number}`}
    >
      {#if previews[index]}
        {@const unpaired = unpairedIn(previews[index])}
        {#each previews[index] as node, at (at)}
          <NodeView {node} {onselect} {unpaired} {onfollow} {onreference} {onfigure} />
        {/each}
        <!--
          Notes gathered for print. `aria-hidden` and hidden on screen: this is
          a second copy of text already on the page, and a screen reader
          reading every footnote twice is worse than one not printed.
        -->
        {@const notes = notesIn(previews[index])}
        {#if notes.length > 0}
          <aside class="usfm-notes usfm-notes-chapter" aria-hidden="true">
            {#each notes as note, at (at)}
              <p class="usfm-notes-entry" class:reference={isReference(note)}>
                {#each note.children as child, childAt (childAt)}
                  <NodeView node={child} {onselect} {unpaired} {onfollow} {onreference} {onfigure} />
                {/each}
              </p>
            {/each}
          </aside>
        {/if}
      {:else}
        <!-- In flight. Deliberately not a spinner: a chapter arrives in a few
             milliseconds, and a spinner that appears and vanishes that fast
             reads as a flicker rather than as progress. -->
        <p class="pending" aria-hidden="true"></p>
      {/if}
    </section>
  {/each}

  {#if allNotes.length > 0}
    <aside class="usfm-notes usfm-notes-document" aria-hidden="true">
      {#each allNotes as note, at (at)}
        <p class="usfm-notes-entry" class:reference={isReference(note)}>
          {#each note.children as child, childAt (childAt)}
            <NodeView node={child} {onselect} {onfollow} {onreference} {onfigure} />
          {/each}
        </p>
      {/each}
    </aside>
  {/if}

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

    /* ARCHITECTURE 10. The browser skips layout, style and paint for a chapter
       that is off screen, which is what makes a two-megabyte document scroll
       rather than crawl. The intrinsic size is what keeps the scrollbar
       roughly honest while that is happening -- without it, every chapter
       claims zero height and the bar jumps as they render.

       Print overrides this, or a printed document is one page long
       (PRODUCT 8). That is P3.10's. */
    content-visibility: auto;
    contain-intrinsic-size: auto 800px;
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


  /* ---------------------------------------------------------- poetry ---
   *
   * `\q1`..`\q4` are indentation levels, and the numbers are open-ended in the
   * specification, so the deepest listed here is a floor rather than a limit:
   * anything deeper simply keeps the previous indent instead of losing it.
   */

  :global(.usfm-q1) { padding-inline-start: 1.5rem; }
  :global(.usfm-q2) { padding-inline-start: 3rem; }
  :global(.usfm-q3) { padding-inline-start: 4.5rem; }
  :global(.usfm-q4) { padding-inline-start: 6rem; }

  :global(.usfm-q1),
  :global(.usfm-q2),
  :global(.usfm-q3),
  :global(.usfm-q4) {
    /* A poetic line that wraps is still one line; the continuation is set in
       from its own indent so the structure survives a narrow pane. */
    text-indent: -0.75rem;
    margin-block-end: 0;
  }

  /* `\qs` is the Selah, set at the end of the line. */
  :global(.usfm-qs) {
    font-style: italic;
    float: inline-end; /* lint-logical-ok: `float` has no logical shorthand */
  }

  /* The blank-line marker: no text, just the space between stanzas. */
  :global(.usfm-b) {
    margin-block: 0.7rem;
    block-size: 0.4em;
  }

  /* ----------------------------------------------------------- lists ---
   *
   * Rendered as paragraphs rather than as `ul`/`li`, because USFM's list
   * markers are a flat sequence of indent levels and not a nested structure —
   * building real nesting from them means inventing a tree the file does not
   * describe, and getting it wrong on the files that skip a level.
   */

  :global(.usfm-li1) { padding-inline-start: 1.5rem; }
  :global(.usfm-li2) { padding-inline-start: 3rem; }
  :global(.usfm-li3) { padding-inline-start: 4.5rem; }
  :global(.usfm-li4) { padding-inline-start: 6rem; }

  :global(.usfm-li1),
  :global(.usfm-li2),
  :global(.usfm-li3),
  :global(.usfm-li4) {
    margin-block-end: 0.2rem;
  }

  /* List header and footer are labels for the list, not items in it. */
  :global(.usfm-lh),
  :global(.usfm-lf) {
    font-style: italic;
    color: var(--text-muted);
    margin-block: 0.5rem 0.2rem;
  }

  /* ---------------------------------------------------------- tables ---
   *
   * `border-collapse` and a light rule under the header, because a table in
   * Scripture is a genealogy or a census and the columns have to line up to be
   * read at all.
   */

  :global(.usfm-table) {
    border-collapse: collapse;
    margin-block: 0.7rem;
    inline-size: 100%;
  }

  :global(.usfm-cell) {
    padding-block: 0.15rem;
    padding-inline: 0.5rem 0.9rem;
    vertical-align: baseline;
  }

  :global(.usfm-align-start) { text-align: start; }
  :global(.usfm-align-center) { text-align: center; }
  :global(.usfm-align-end) { text-align: end; }

  :global(.usfm-table th.usfm-cell) {
    font-weight: 600;
    border-block-end: 1px solid var(--border);
  }

  /* --------------------------------------------------------- sidebar ---
   *
   * `\esb` is an aside in the reading sense as well as the markup one, so it
   * is set apart rather than inline with the Scripture it sits beside.
   */

  :global(.usfm-sidebar) {
    border-inline-start: 3px solid var(--border);
    padding-inline-start: 0.9rem;
    margin-block: 0.9rem;
    background: color-mix(in srgb, var(--surface-sunken) 60%, transparent);
    padding-block: 0.5rem;
  }

  /* ---------------------------------------------------------- figure ---
   *
   * Images are off by default (SECURITY 3), so the placeholder is what is
   * normally seen and it has to be useful: the caption and the reference are
   * the parts a reader actually needs, and a broken-image box carries neither.
   */

  /* Print only. On screen these are a duplicate of text already shown. */
  :global(.usfm-notes) {
    display: none;
  }

  :global(.usfm-figure) {
    margin-block: 0.9rem;
    margin-inline: 0;
  }

  :global(.usfm-figure-frame) {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    align-items: center;
    justify-content: center;
    min-block-size: 4rem;
    padding: 0.75rem;
    border: 1px dashed var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 0.85em;
  }

  :global(.usfm-figure-src) {
    font-family: var(--font-gutter);
    font-size: 0.9em;
    /* A file name from a document, which may be in any script and any
       direction; it must not rearrange the sentence around it. */
    unicode-bidi: isolate;
  }

  /* The loaded image. Bounded rather than natural size: a figure in a
     translation file can be any dimensions at all, and one at its own scale
     would push the Scripture it belongs to off the screen. */
  :global(.usfm-figure-image) {
    max-inline-size: 100%;
    /* Half the pane, so an image never takes the whole reading column. */
    max-block-size: 50vh;
    block-size: auto;
    object-fit: contain;
  }

  /* The per-document opt-in, offered on the placeholder itself. Quiet: it is
     an offer, not the thing the reader came for. */
  :global(.usfm-figure-show) {
    margin-block-start: 0.35rem;
    padding-block: 0.15rem;
    padding-inline: 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: none;
    color: inherit;
    font: inherit;
    font-size: 0.95em;
    cursor: pointer;
  }

  :global(.usfm-figure-show:hover) {
    border-color: var(--accent);
    color: var(--text);
  }

  :global(.usfm-figure figcaption) {
    font-size: 0.9em;
    color: var(--text-muted);
    margin-block-start: 0.3rem;
  }

  :global(.usfm-figure-ref) {
    margin-inline-start: 0.4em;
    font-style: italic;
  }

  /* ------------------------------------------------------- milestone ---
   *
   * Only ever seen when unpaired (PRODUCT 7). A chip, so the reader can see
   * exactly where the markup stopped making sense without the rest of the
   * document disappearing into it.
   */

  :global(.usfm-milestone) {
    display: inline-block;
    font-family: var(--font-gutter);
    font-size: 0.75em;
    padding-inline: 0.35em;
    border-radius: 3px;
    background: color-mix(in srgb, var(--severity-warning) 25%, transparent);
    color: var(--text);
    unicode-bidi: isolate;
  }

  /* ----------------------------------------------------------- links ---
   *
   * Three outcomes from SECURITY 2, and they must not look alike: one opens
   * something outside the application, one moves the cursor, and one does
   * nothing at all and needs to say so.
   */

  :global(.usfm-link),
  :global(.usfm-ref) {
    font: inherit;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
    color: var(--accent);
    text-decoration: underline;
  }

  :global(.usfm-ref) {
    color: var(--syntax-verse);
  }

  :global(.usfm-inert) {
    /* Not a link, and deliberately not styled as one. The wavy underline is
       the same language the diagnostics use for "this is wrong". */
    text-decoration: underline wavy var(--severity-warning);
    text-underline-offset: 0.2em;
    cursor: help;
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
