<script lang="ts">
  /**
   * One document node, and its children.
   *
   * The recursion at the centre of the preview. Every kind the specification
   * defines arrives here and leaves as elements — never as a string, never
   * through Svelte's raw-markup directive. SECURITY §1 puts the control in
   * there being *no path* from document content to raw markup, and this file
   * is where that path would be if it existed: the node carries `text` and
   * `raw`, both of which go into text positions where Svelte escapes them,
   * and `attributes`, which are read by key rather than spread.
   *
   * The directive is not named literally here on purpose — the lint that
   * enforces the ban has no exemption, so the ban covers the sentence
   * describing it too. That is the correct trade: a rule that can be waved
   * through in a comment is a rule that can be waved through.
   *
   * # The fallthrough is the point
   *
   * A kind this component does not know renders its children anyway. USFM has
   * markers this editor has never heard of and files that predate half of it,
   * and ADR-003 says content survives — so an unhandled kind must lose its
   * styling, not its Scripture. P3.2 and P3.3 add notes, poetry and tables by
   * giving those kinds their own arms; until then they render as plain
   * paragraphs rather than as nothing.
   */

  import type { PreviewNode } from "../../worker/protocol";
  import Self from "./NodeView.svelte";

  interface Props {
    node: PreviewNode;
    /** Clicking any node moves the editor cursor there (PRODUCT §7). */
    onselect?: (start: number, end: number) => void;
  }

  let { node, onselect }: Props = $props();

  const attribute = (key: string): string | undefined =>
    node.attributes.find((entry) => entry.key === key)?.value;

  /**
   * The class for a marked node.
   *
   * Derived from the marker rather than enumerated, so `\q1`, `\q2`, `\li3`
   * and every level the specification numbers open-endedly get a class without
   * this file listing them. Restricted to what a marker can legally be, since
   * a class name is being built from document content.
   */
  const marked = $derived(
    node.marker && /^[+]?[A-Za-z0-9-]+$/.test(node.marker)
      ? `usfm-${node.marker.replace(/^\+/, "")}`
      : "usfm-unknown",
  );

  /**
   * Where in the source this node is, when the parser recorded it.
   *
   * The click stops here. Nodes nest -- a verse sits inside a paragraph, a
   * character style inside a verse -- so without this the event bubbles to
   * every ancestor and the outermost handler runs last and wins. Clicking
   * verse two put the cursor at the top of the paragraph, which reported the
   * chapter; the innermost node is the one the user aimed at.
   */
  function select(event: MouseEvent): void {
    if (node.start === null || node.end === null) return;
    event.stopPropagation();
    onselect?.(node.start, node.end);
  }
</script>

{#if node.kind === "text"}
  <!-- A text position. Svelte escapes it; there is no other option here. -->
  {node.text}
{:else if node.raw !== null}
  <!--
    Content the parser could not classify, shown verbatim rather than dropped
    (ADR-003). Marked so it is visibly not ordinary Scripture — a placeholder
    that looked like text would be the preview quietly asserting that malformed
    markup is fine.
  -->
  <span class="usfm-raw" title="This markup could not be interpreted">{node.raw}</span>
{:else if node.kind === "chapter"}
  <h2 class="usfm-chapter" onclick={select} role="presentation">
    {attribute("number") ?? ""}
  </h2>
{:else if node.kind === "verse"}
  <!--
    The number, not the verse. USFM's `\v` marks a *position*, and the text
    that follows is a sibling rather than a child — so a verse renders as its
    number and the words after it flow on in the paragraph, which is how
    Scripture is set.
  -->
  <span
    class="usfm-verse"
    data-verse={attribute("number") ?? ""}
    onclick={select}
    role="presentation"
  >{attribute("pubnumber") ?? attribute("number") ?? ""}</span>
{:else if node.kind === "para"}
  <p class="usfm-para {marked}" onclick={select} role="presentation">
    {#each node.children as child, index (index)}
      <Self node={child} {onselect} />
    {/each}
  </p>
{:else if node.kind === "char"}
  <span class="usfm-char {marked}">
    {#each node.children as child, index (index)}
      <Self node={child} {onselect} />
    {/each}
  </span>
{:else if node.kind === "book"}
  <!-- `\id` is metadata, not Scripture. It belongs in the status bar, and
       showing it at the top of the reading pane would put a book code where
       the title goes. -->
{:else if node.kind === "optbreak"}
  <wbr />
{:else}
  <!-- Everything not yet given its own arm: still rendered, still readable. -->
  <span class="usfm-other {marked}">
    {#each node.children as child, index (index)}
      <Self node={child} {onselect} />
    {/each}
  </span>
{/if}
