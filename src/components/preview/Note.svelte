<script lang="ts">
  /**
   * A footnote or cross-reference — P3.2.
   *
   * `\f`, `\fe`, `\ef`, `\efe`, `\x`, `\ex`. All six are one shape: a caller
   * in the text, and content that belongs to it.
   *
   * # Why the content is inline and collapsed
   *
   * A reading preview is not a printed page. Notes at the foot of a scrolling
   * pane would be nowhere near what they annotate, and a hover popover is
   * unusable by keyboard and on touch. A caller that expands in place is
   * reachable every way, needs no positioning, and keeps the note beside the
   * word it is about — which is what the reader wants and what the translator
   * checking it needs.
   *
   * Collapsed by default because a chapter with a note per verse is a chapter
   * whose Scripture is buried in apparatus.
   */

  import type { Snippet } from "svelte";

  interface Props {
    /**
     * The caller as the file gives it.
     *
     * `+` means "generate one", `-` means "no caller", and anything else is
     * literal. The specification lets a file choose, and a preview that always
     * printed `*` would be showing something the document does not say.
     */
    caller: string;
    /** `f`, `fe`, `x`, `ex` — which kind of note this is. */
    marker: string;
    /** Clicking the note moves the editor cursor to it. */
    onselect?: (event: MouseEvent) => void;
    children: Snippet;
  }

  let { caller, marker, onselect, children }: Props = $props();

  let open = $state(false);

  /** Cross-references are `\x`/`\ex`; everything else is a footnote. */
  const isReference = $derived(marker.startsWith("x") || marker === "ex");

  /**
   * What the caller shows.
   *
   * `-` asks for no caller at all, but a note with nothing to click cannot be
   * opened — so it gets the generated mark. Losing access to the note is a
   * worse failure than showing a mark the file did not ask for, and this is a
   * preview rather than a typesetter.
   */
  const shown = $derived(caller === "+" || caller === "-" || caller === "" ? "*" : caller);

  const label = $derived(
    `${isReference ? "Cross reference" : "Footnote"}, ${open ? "expanded" : "collapsed"}`,
  );
</script>

<span class="usfm-note" class:reference={isReference}>
  <button
    type="button"
    class="usfm-caller"
    aria-expanded={open}
    aria-label={label}
    onclick={(event) => {
      event.stopPropagation();
      open = !open;
      onselect?.(event);
    }}
  >{shown}</button>

  {#if open}
    <!--
      `role="note"` rather than a generic span: a screen reader announcing this
      as an aside is the difference between apparatus and Scripture, and a
      reader who cannot see the smaller type has no other cue.
    -->
    <span class="usfm-note-body" role="note">{@render children()}</span>
  {/if}
</span>

<style>
  .usfm-note {
    /* The caller belongs to the word before it, so the pair must not be split
       across a line break. */
    white-space: normal;
  }

  .usfm-caller {
    font: inherit;
    font-size: 0.7em;
    vertical-align: super;
    line-height: 1;
    padding: 0;
    margin-inline: 0.1em;
    border: none;
    background: none;
    color: var(--accent);
    cursor: pointer;
    /* A Latin caller inside a right-to-left verse belongs where it was
       written (UNICODE §8). */
    unicode-bidi: isolate;
  }

  .usfm-caller:hover {
    text-decoration: underline;
  }

  .usfm-note-body {
    display: inline;
    font-size: 0.9em;
    color: var(--text-muted);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    padding-inline: 0.35em;
    border-radius: 3px;
    /* Isolated as a unit: a note is a parenthesis in the reading, and its
       direction must not leak into the verse around it. */
    unicode-bidi: isolate;
  }

  .reference .usfm-note-body {
    background: color-mix(in srgb, var(--syntax-milestone) 12%, transparent);
  }
</style>
