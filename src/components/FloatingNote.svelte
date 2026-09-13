<script lang="ts">
  /**
   * The note inspector, floated beside the line it describes.
   *
   * Study has no inspector column, so the apparatus comes to the text instead
   * of the other way round: put the caret in a footnote and the card appears
   * next to it. That is the whole difference between the two layouts' handling
   * of notes — same component inside, same fields, different place.
   *
   * # It is positioned, not placed
   *
   * The card follows the caret's own coordinates and is then pushed back
   * inside the pane, because a card that hangs off the right edge of a narrow
   * editor is a card with half its buttons unreachable. Written through the
   * CSSOM rather than as a `style` attribute: `style-src` without
   * `'unsafe-inline'` blocks inline style attributes as well as style elements
   * (SECURITY §4), so a `style:` directive here is a console violation every
   * time the caret moves.
   */

  import type { NoteEntry } from "../lib/notes";
  import NotePanel from "./NotePanel.svelte";

  interface Props {
    notes: NoteEntry[];
    selected: number;
    /** Where the note begins, in the editor's own coordinates. */
    at: { x: number; y: number; bottom: number } | null;
    readOnly: boolean;
    onselect: (index: number) => void;
    ongo: (offset: number) => void;
    onconvert: (note: NoteEntry) => void;
    onclose: () => void;
  }

  let { notes, selected, at, readOnly, onselect, ongo, onconvert, onclose }: Props = $props();

  /** The card's own width, which the clamping needs to know. */
  const WIDTH = 340;
  const GAP = 8;

  let card = $state<HTMLElement>();

  $effect(() => {
    const element = card;
    if (!element || !at) return;

    const pane = element.offsetParent as HTMLElement | null;
    const room = pane?.clientWidth ?? WIDTH + GAP * 2;
    const tall = pane?.clientHeight ?? 0;

    const x = Math.max(GAP, Math.min(at.x, room - WIDTH - GAP));
    // Under the line by preference, above it when there is no room under.
    const below = at.bottom + GAP;
    const fits = tall === 0 || below + element.offsetHeight + GAP < tall;
    const y = fits ? below : Math.max(GAP, at.y - element.offsetHeight - GAP);

    element.style.setProperty("--x", `${x}px`);
    element.style.setProperty("--y", `${y}px`);
  });

  /**
   * Escape closes it, listened for on the element rather than declared on it.
   *
   * A `div` with a keyboard handler is a non-interactive element listening for
   * keys, which is both a lint failure and a fair description of the problem:
   * the card is a container, and the keys reach it only because one of the
   * buttons inside has focus. Attaching the listener says that plainly.
   */
  $effect(() => {
    const element = card;
    if (!element) return;

    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      // Stopped, so the editor's own Escape handling does not also run.
      event.stopPropagation();
      onclose();
    };

    element.addEventListener("keydown", escape);
    return () => element.removeEventListener("keydown", escape);
  });
</script>

{#if at && notes[selected]}
  <!--
    Not a dialog: the editor keeps focus and typing continues. A modal here
    would mean the note could only be read by stopping work, which is the
    opposite of what floating it beside the caret is for.
  -->
  <div
    class="card"
    bind:this={card}
    role="group"
    aria-label="Note at the caret"
    tabindex="-1"
  >
    <NotePanel {notes} {selected} {onselect} {ongo} {onconvert} {readOnly} compact />
  </div>
{/if}

<style>
  .card {
    position: absolute;
    inset-block-start: var(--y, 0);
    inset-inline-start: var(--x, 0);
    inline-size: 340px;
    max-inline-size: calc(100% - 16px);
    z-index: 20;
    background: var(--bg2);
    border: 1px solid var(--line);
    border-radius: 6px;
    box-shadow: var(--shadow);
    /* The card is chrome sitting over Scripture; it takes the interface face
       rather than the reading one. */
    font-family: var(--font-ui);
  }
</style>
