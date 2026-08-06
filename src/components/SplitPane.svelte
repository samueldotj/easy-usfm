<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { isNumber, read, write } from "../lib/settings";

  interface Props {
    /** Persistence key, so different splits remember themselves separately. */
    id: string;
    start: Snippet;
    end: Snippet;
    startLabel: string;
    endLabel: string;
  }

  let { id, start, end, startLabel, endLabel }: Props = $props();

  const key = $derived(`split.${id}`);
  const MIN = 15;
  const MAX = 85;
  const STEP = 2;

  const clamp = (percent: number) => Math.min(MAX, Math.max(MIN, percent));

  // The stored position is read once, at startup. `untrack` says so out loud:
  // this is the initial value on purpose, not a reactive read someone forgot
  // to wire up. Re-reading storage as the user drags would fight them.
  let percent = $state(clamp(untrack(() => read(`split.${id}`, 50, isNumber))));
  let container: HTMLDivElement;

  /**
   * The divider position, written through the CSSOM rather than as a `style`
   * attribute.
   *
   * `style-src` without `'unsafe-inline'` blocks inline style *attributes* as
   * well as style elements (SECURITY §4), so `style="flex-basis: …"` is a
   * console violation on every render under the real policy. Assigning a
   * custom property through the CSSOM is not inline style and is not blocked —
   * and one property on the container beats two attributes on the panes.
   */
  $effect(() => {
    container?.style.setProperty("--split", `${percent}%`);
  });
  let dragging = $state(false);

  function positionFrom(clientX: number): void {
    const box = container.getBoundingClientRect();
    if (box.width === 0) return;
    percent = clamp(((clientX - box.left) / box.width) * 100);
  }

  function onPointerDown(event: PointerEvent): void {
    dragging = true;
    const divider = event.currentTarget as HTMLElement;

    // Capture, so a fast drag that outruns the pointer keeps resizing rather
    // than stopping the moment the cursor leaves the five-pixel divider.
    divider.setPointerCapture(event.pointerId);

    // preventDefault stops the panes' text being selected mid-drag, and also
    // stops the divider taking focus -- so focus is given explicitly. Without
    // this, clicking the divider and then pressing an arrow key does nothing
    // at all, which reads as the keyboard resize being broken.
    event.preventDefault();
    divider.focus();
  }

  function onPointerMove(event: PointerEvent): void {
    if (dragging) positionFrom(event.clientX);
  }

  function onPointerUp(event: PointerEvent): void {
    if (!dragging) return;
    dragging = false;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
    write(key, percent);
  }

  /**
   * The divider is operable from the keyboard.
   *
   * PRODUCT §10 designs accessibility in from Phase 1 rather than retrofitting
   * it, on the grounds that a split-pane editor retrofitted late "technically
   * passes and is unusable". A divider that can only be dragged is the
   * clearest example: without this, a keyboard-only user cannot change the
   * layout at all.
   */
  function onKeyDown(event: KeyboardEvent): void {
    const move =
      event.key === "ArrowLeft" ? -STEP : event.key === "ArrowRight" ? STEP : 0;

    if (move !== 0) {
      percent = clamp(percent + move);
    } else if (event.key === "Home") {
      percent = MIN;
    } else if (event.key === "End") {
      percent = MAX;
    } else if (event.key === "Enter") {
      percent = 50;
    } else {
      return;
    }

    event.preventDefault();
    write(key, percent);
  }
</script>

<div class="split" bind:this={container}>
  <!-- `data-pane` is what F6 cycles over: a pane that exists is a pane F6
       reaches, rather than one somebody remembered to add to a list.

       `tabindex="-1"` is what makes that work. A section is not focusable, so
       calling focus() on a pane whose contents are not either does nothing at
       all -- F6 appeared to be stuck in the editor because moving to the
       preview silently failed and left focus where it was. -1 keeps it out of
       the tab order while allowing the programmatic move. -->
  <section
    class="pane"
    data-pane
    tabindex="-1"
    aria-label={startLabel}
  >
    {@render start()}
  </section>

  <!--
    A focusable separator is the ARIA window-splitter pattern, not a mistake:
    a divider that can only be dragged cannot be operated from the keyboard at
    all, which is exactly the "technically passes and is unusable" outcome
    PRODUCT §10 warns about. The linter cannot tell the two apart.
  -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="divider"
    class:dragging
    role="separator"
    tabindex="0"
    aria-orientation="vertical"
    aria-label="Resize panes"
    aria-valuenow={Math.round(percent)}
    aria-valuemin={MIN}
    aria-valuemax={MAX}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    ondblclick={() => {
      percent = 50;
      write(key, percent);
    }}
    onkeydown={onKeyDown}
  ></div>

  <section
    class="pane"
    data-pane
    tabindex="-1"
    aria-label={endLabel}
  >
    {@render end()}
  </section>
</div>

<style>
  .split {
    display: flex;
    block-size: 100%;
    min-block-size: 0;
  }

  .pane {
    min-inline-size: 0;
    overflow: hidden;
  }

  /* The two shares of `--split`, which the effect above sets through the
     CSSOM. Selected by position rather than by a class, because there are
     exactly two panes and their order is the layout. */
  .pane:first-of-type {
    flex-basis: var(--split, 50%);
  }

  .pane:last-of-type {
    flex-basis: calc(100% - var(--split, 50%));
  }

  .divider {
    flex: 0 0 auto;
    inline-size: 5px;
    cursor: col-resize;
    background: var(--border);
    /* A five-pixel target is hard to hit and impossible on a touchscreen, so
       the hit area is widened without widening the line. */
    border-inline: 3px solid transparent;
    background-clip: padding-box;
    box-sizing: content-box;
    touch-action: none;
  }

  .divider:hover,
  .divider.dragging {
    background: var(--accent);
  }

  .divider:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
