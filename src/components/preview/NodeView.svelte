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
   * styling, not its Scripture.
   */

  import { sanitizeHref } from "../../lib/href";
  import type { PreviewNode } from "../../worker/protocol";
  import Note from "./Note.svelte";
  import Self from "./NodeView.svelte";

  interface Props {
    node: PreviewNode;
    /** Clicking any node moves the editor cursor there (PRODUCT §7). */
    onselect?: (start: number, end: number) => void;
    /** Milestones with no partner, which render as a chip (P3.4). */
    unpaired?: ReadonlySet<PreviewNode>;
    /** A link the user chose to follow. Never opened in the webview. */
    onfollow?: (href: string) => void;
    /** A scripture reference in a link, resolved rather than navigated. */
    onreference?: (reference: string) => void;
  }

  let { node, onselect, unpaired, onfollow, onreference }: Props = $props();

  /** Everything the recursion passes straight through. */
  const pass = $derived({ onselect, unpaired, onfollow, onreference });

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

  /** Where in the source this node is, when the parser recorded it. */
  function select(event: MouseEvent): void {
    if (node.start === null || node.end === null) return;
    // The click stops here. Nodes nest — a verse sits inside a paragraph, a
    // character style inside a verse — so without this the event bubbles to
    // every ancestor and the outermost handler runs last and wins. Clicking
    // verse two put the cursor at the top of the paragraph, which reported the
    // chapter; the innermost node is the one the user aimed at.
    event.stopPropagation();
    onselect?.(node.start, node.end);
  }

  /**
   * A character marker's link target, if it carries one (SECURITY §2).
   *
   * `\jmp` is the marker for it, but `link-href` is generic on character
   * markers, so this asks every one of them rather than only `\jmp`.
   */
  const link = $derived.by(() => {
    const href = node.attributes.find((entry) => entry.key === "link-href")?.value;
    return href === undefined ? null : sanitizeHref(href);
  });

  const isUnpaired = $derived(unpaired?.has(node) ?? false);

  /**
   * A table cell's alignment, as a class rather than a style attribute.
   *
   * `style-src` without `'unsafe-inline'` blocks inline style attributes as
   * well as style elements (SECURITY 4), so a `style:` directive here is a
   * console violation on every table. Restricted to the three logical values
   * the parser emits, since a class name is being built from document content.
   */
  const align = $derived.by(() => {
    const value = node.attributes.find((entry) => entry.key === "align")?.value;
    return value === "center" || value === "end" ? value : "start";
  });
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
{:else if node.kind === "note"}
  <Note caller={attribute("caller") ?? "*"} marker={node.marker ?? "f"} onselect={select}>
    {#each node.children as child, index (index)}
      <Self node={child} {...pass} />
    {/each}
  </Note>
{:else if node.kind === "para"}
  <p class="usfm-para {marked}" onclick={select} role="presentation">
    {#each node.children as child, index (index)}
      <Self node={child} {...pass} />
    {/each}
  </p>
{:else if node.kind === "char" && link !== null}
  <!--
    A link out of a document that arrived from a third party (SECURITY §2).
    Three outcomes, and only one of them opens anything.
  -->
  {#if link.kind === "external"}
    <button
      type="button"
      class="usfm-char usfm-link {marked}"
      title={link.value}
      onclick={(event) => {
        event.stopPropagation();
        onfollow?.(link.value);
      }}
    >
      {#each node.children as child, index (index)}
        <Self node={child} {...pass} />
      {/each}
    </button>
  {:else if link.kind === "ref"}
    <button
      type="button"
      class="usfm-char usfm-ref {marked}"
      title="Go to {link.value}"
      onclick={(event) => {
        event.stopPropagation();
        onreference?.(link.value);
      }}
    >
      {#each node.children as child, index (index)}
        <Self node={child} {...pass} />
      {/each}
    </button>
  {:else}
    <!--
      Not an anchor and not a button: the application does not act on this at
      all. The text is shown so the reader can see what the file contained,
      which is more useful than a silently removed link and is the only outcome
      that tells them their document is odd.
    -->
    <span
      class="usfm-char usfm-inert {marked}"
      title="This link was not safe to follow: {link.value}"
    >
      {#each node.children as child, index (index)}
        <Self node={child} {...pass} />
      {/each}
    </span>
  {/if}
{:else if node.kind === "char"}
  <span class="usfm-char {marked}">
    {#each node.children as child, index (index)}
      <Self node={child} {...pass} />
    {/each}
  </span>
{:else if node.kind === "table"}
  <table class="usfm-table" onclick={select} role="presentation">
    <tbody>
      {#each node.children as child, index (index)}
        <Self node={child} {...pass} />
      {/each}
    </tbody>
  </table>
{:else if node.kind === "table:row"}
  <tr class="usfm-row">
    {#each node.children as child, index (index)}
      <Self node={child} {...pass} />
    {/each}
  </tr>
{:else if node.kind === "table:cell"}
  <!--
    A heading cell is `\th`, a body cell `\tc`. The distinction is in the
    marker rather than in the kind, which is USJ's model, so it is read from
    there — a table whose headers were ordinary cells reads as data with no
    column names.

    `align` arrives from the parser as a logical value already, so it goes
    straight to `text-align` (UNICODE §8).
  -->
  {#if node.marker?.startsWith("th")}
    <th class="usfm-cell {marked} usfm-align-{align}">
      {#each node.children as child, index (index)}
        <Self node={child} {...pass} />
      {/each}
    </th>
  {:else}
    <td class="usfm-cell {marked} usfm-align-{align}">
      {#each node.children as child, index (index)}
        <Self node={child} {...pass} />
      {/each}
    </td>
  {/if}
{:else if node.kind === "sidebar"}
  <!-- `\esb` is an aside in the reading sense as well as the markup one. -->
  <aside class="usfm-sidebar" onclick={select} role="presentation">
    {#each node.children as child, index (index)}
      <Self node={child} {...pass} />
    {/each}
  </aside>
{:else if node.kind === "figure"}
  <!--
    Images are off by default (SECURITY §3), so what renders is the caption and
    what the file asked for. A figure is not decoration in Scripture — it
    carries a caption and a reference the reader needs — so the placeholder
    shows those rather than a broken-image box.
  -->
  <figure class="usfm-figure" onclick={select} role="presentation">
    <div class="usfm-figure-frame">
      <span class="usfm-figure-note">Image not shown</span>
      {#if attribute("file")}
        <span class="usfm-figure-src">{attribute("file")}</span>
      {/if}
    </div>
    <figcaption>
      {#each node.children as child, index (index)}
        <Self node={child} {...pass} />
      {/each}
      {#if attribute("ref")}
        <span class="usfm-figure-ref">{attribute("ref")}</span>
      {/if}
    </figcaption>
  </figure>
{:else if node.kind === "ms"}
  <!--
    A milestone marks a position and normally renders as nothing — that is what
    milestones are for. An *unpaired* one is different: PRODUCT §7 says it
    warns and renders as a chip rather than swallowing the rest of the
    document, so the reader can see where the markup stops making sense.
  -->
  {#if isUnpaired}
    <span
      class="usfm-milestone"
      onclick={select}
      role="presentation"
      title="This milestone has no matching partner"
    >{node.marker}</span>
  {/if}
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
      <Self node={child} {...pass} />
    {/each}
  </span>
{/if}
