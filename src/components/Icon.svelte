<script lang="ts">
  /**
   * The interface's icons, drawn here.
   *
   * Inline SVG, not a font and not a sprite fetched at runtime: a font is a
   * network request the offline build must not make (PRODUCT §12), and a
   * fetched sprite is blocked by the policy outright (SECURITY §4). Lucide's
   * shapes, at Lucide's proportions — 24×24, stroked, round caps — because
   * that is what the design system specifies and they weigh the same beside
   * the interface's type.
   *
   * `currentColor` throughout, so an icon follows the theme and its own
   * button's state without a second copy for dark mode.
   *
   * Always `aria-hidden`. An icon here is never the only label on a control;
   * whatever carries it carries the words too.
   */

  interface Props {
    name:
      | "search"
      | "editor"
      | "split"
      | "preview"
      | "sync"
      | "theme"
      | "collapse"
      | "expand"
      | "plus"
      | "close"
      | "next"
      | "previous"
      | "jump"
      | "warning"
      | "wrap"
      | "print";
    /** In `em`, so an icon scales with the text it sits beside. */
    size?: number;
  }

  let { name, size = 1 }: Props = $props();
</script>

<svg
  viewBox="0 0 24 24"
  aria-hidden="true"
  focusable="false"
  width="{size}em"
  height="{size}em"
>
  {#if name === "search"}
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.3-4.3" />
  {:else if name === "editor"}
    <!-- A pane with its divider at the left: source only. -->
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <path d="M9 3v18" />
  {:else if name === "split"}
    <rect x="3" y="3" width="18" height="18" rx="2" />
    <path d="M12 3v18" />
  {:else if name === "preview"}
    <path d="M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" />
    <circle cx="12" cy="12" r="3" />
  {:else if name === "sync"}
    <!-- Two links of a chain: the panes are joined. -->
    <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" />
    <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" />
  {:else if name === "theme"}
    <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
  {:else if name === "collapse"}
    <path d="m15 18-6-6 6-6" />
  {:else if name === "expand"}
    <path d="m9 18 6-6-6-6" />
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "close"}
    <path d="M18 6 6 18M6 6l12 12" />
  {:else if name === "next"}
    <path d="m9 6 6 6-6 6" />
  {:else if name === "previous"}
    <path d="m15 6-6 6 6 6" />
  {:else if name === "jump"}
    <path d="M5 12h14M13 6l6 6-6 6" />
  {:else if name === "warning"}
    <path d="M12 3 2 20h20L12 3Z" />
    <path d="M12 10v4M12 17.5v.01" />
  {:else if name === "wrap"}
    <!-- Text that turns back on itself: the invisible-characters toggle. -->
    <path d="M4 6h16M4 12h13a3 3 0 0 1 0 6h-3M9 15l-3 3 3 3" />
  {:else if name === "print"}
    <path d="M6 9V3h12v6M6 18H4v-6h16v6h-2" />
    <path d="M6 14h12v7H6z" />
  {/if}
</svg>

<style>
  svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
    flex: 0 0 auto;
  }
</style>
