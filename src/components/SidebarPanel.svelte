<script lang="ts">
  /**
   * Study sidebars, as records rather than as markup.
   *
   * `\esb … \esbe` is the one construct in USFM that is really a small form:
   * a category, a heading and a body, written the same way hundreds of times
   * in a study Bible. Editing it in the source means counting backslashes; the
   * panel gives it three fields and a list of the ones already in this chapter.
   *
   * # Apply, not autosave
   *
   * The button is explicit and it is called "Apply to source", because that is
   * what it does: it replaces a span of the document through the editor's own
   * transaction, so it is one undo step and the buffer stays the authority
   * (ADR-003). A form that wrote as you typed would produce one undo entry per
   * keystroke in a text box, which is unusable the first time somebody wants
   * their paragraph back.
   *
   * # The form follows the document until somebody edits the form
   *
   * Both halves of that matter, and they pull in opposite directions — see the
   * effect below, which is where the rule is written down.
   */

  import { untrack } from "svelte";

  import { changed, rewrite, wordCount, type Sidebar, type SidebarEdit } from "../lib/sidebars";

  interface Props {
    sidebars: Sidebar[];
    /** Which chapter these belong to, for the count line. */
    chapter: string | null;
    /** The reference a new sidebar would be inserted after. */
    newAt: string | null;
    /** Nothing can be edited in a document another window holds. */
    readOnly: boolean;
    /** Replaces a span of the document. */
    onapply: (from: number, to: number, text: string) => void;
    /** Puts the caret at an offset. */
    ongo: (offset: number) => void;
    /** Inserts a new sidebar at the caret. */
    onnew: () => void;
  }

  let { sidebars, chapter, newAt, readOnly, onapply, ongo, onnew }: Props = $props();

  /** Common `\cat` values. Suggestions, not a closed set: `\cat` is free text. */
  const CATEGORIES = [
    "Word Study",
    "Theology",
    "Background",
    "Culture",
    "Geography",
    "People",
    "History",
    "Application",
  ];

  let selected = $state(0);

  const sidebar = $derived(sidebars[Math.min(selected, Math.max(0, sidebars.length - 1))]);

  /** The form. Reloaded when the selection moves to a different block. */
  let category = $state("");
  let heading = $state("");
  let body = $state("");

  /**
   * What counts as "a different block": where it is, not what it says.
   *
   * The content changes on every keystroke in the document, and reloading on
   * that alone would empty a field somebody is typing into.
   */
  const identity = $derived(sidebar ? `${sidebar.index}:${sidebar.start}` : "none");

  /** The fields as the document has them. */
  function fieldsOf(block: Sidebar | undefined) {
    return {
      category: block?.category ?? "",
      heading: block?.heading ?? "",
      // Paragraphs as blank-line-separated text, which is how everyone writes
      // paragraphs into a box. Split back the same way on the way out.
      body: (block?.paragraphs ?? []).join("\n\n"),
    };
  }

  let loadedId = "";
  let loadedFields = fieldsOf(undefined);

  /**
   * The form follows the document until somebody starts editing the form.
   *
   * Two ways this has to behave, and they pull in opposite directions. Editing
   * the heading in the *source* has to show up here, or the panel describes a
   * sidebar that no longer exists — which is what happens if the form only
   * reloads when the selection moves. Editing it in the *form* must not be
   * undone by the next parse.
   *
   * So: reload when the selection moves, and otherwise only while the form
   * still holds exactly what it was given. The moment a character is typed
   * into a field, the form is the user's and the document stops overwriting it
   * until Apply or a different sidebar.
   *
   * The field values are read untracked, so this depends on the document and
   * not on itself — an effect that both reads and writes the same state is one
   * keystroke away from a loop.
   */
  $effect(() => {
    const fresh = fieldsOf(sidebar);
    const id = identity;

    untrack(() => {
      const pristine =
        category === loadedFields.category &&
        heading === loadedFields.heading &&
        body === loadedFields.body;
      if (id === loadedId && !pristine) return;

      loadedId = id;
      loadedFields = fresh;
      category = fresh.category;
      heading = fresh.heading;
      body = fresh.body;
    });
  });

  const edit = $derived<SidebarEdit>({
    category: category.trim() || null,
    heading: heading.trim() || null,
    paragraphs: body
      .split(/\n{2,}/)
      .map((paragraph) => paragraph.replace(/\s*\n\s*/g, " ").trim())
      // An empty box is one empty paragraph rather than none, so a sidebar
      // being written keeps somewhere to type.
      .filter((paragraph, _at, all) => paragraph !== "" || all.length === 1),
  });

  const dirty = $derived(sidebar ? changed(sidebar, edit) : false);
  const words = $derived(wordCount(edit.paragraphs));

  function apply(): void {
    if (!sidebar || readOnly) return;
    onapply(sidebar.start, sidebar.end, rewrite(sidebar, edit));
  }

  /**
   * Removing takes the whole block and the newline after it.
   *
   * Leaving the newline would leave a blank line where the sidebar was, which
   * in USFM is not nothing — it is a line the next parse has to decide about.
   */
  function remove(): void {
    if (!sidebar || readOnly) return;
    const confirmed = confirm(
      `Remove this sidebar${sidebar.heading ? ` — “${sidebar.heading}”` : ""}?\n\n` +
        "Its heading and text are deleted from the document. Undo will bring it back.",
    );
    if (!confirmed) return;
    onapply(sidebar.start, sidebar.end + 1, "");
  }
</script>

<div class="panel">
  <div class="summary">
    <span class="markers">\esb … \esbe</span>
    <span>
      · {sidebars.length}
      {sidebars.length === 1 ? "sidebar" : "sidebars"}
      {#if chapter}in chapter {chapter}{/if}
    </span>
    <button type="button" class="new" disabled={readOnly} onclick={onnew}>
      + New{#if newAt}&nbsp;at {newAt}{/if}
    </button>
  </div>

  {#if sidebars.length > 0}
    <ul class="list">
      {#each sidebars as entry (entry.start)}
        <li>
          <button
            type="button"
            class="entry"
            class:on={entry.index === selected}
            aria-current={entry.index === selected ? "true" : undefined}
            onclick={() => (selected = entry.index)}
            ondblclick={() => ongo(entry.start)}
          >
            <span class="ref">{entry.after ?? "—"}</span>
            <span class="heading">{entry.heading || "Untitled sidebar"}</span>
            <span class="cat">{entry.category ?? ""}</span>
            <span class="lines">L{entry.firstLine}–{entry.lastLine}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if sidebar}
    <div class="form">
      <div class="where">
        <span class="strong">Editing sidebar</span>
        {#if sidebar.after}<span>after</span><span class="anchor">{sidebar.after}</span>{/if}
        <span class="span">L{sidebar.firstLine}–{sidebar.lastLine}</span>
      </div>

      {#if sidebar.otherLines > 0}
        <!--
          Said out loud rather than quietly handled. The rewrite keeps those
          lines exactly where they are, and a translator who has put poetry in
          a study note deserves to know the form is not the whole of it.
        -->
        <p class="note">
          This sidebar also contains {sidebar.otherLines}
          {sidebar.otherLines === 1 ? "line" : "lines"} of other markup. It is kept exactly as it is.
        </p>
      {/if}

      <label class="field">
        <span class="label"><code>\cat</code> Category</span>
        <input
          bind:value={category}
          type="text"
          list="sidebar-categories"
          autocomplete="off"
          spellcheck="false"
          disabled={readOnly}
          placeholder="None"
        />
      </label>

      <!-- `\cat` is free text in the specification, so these are suggestions
           and not a closed list. A select here would refuse a category some
           project has been using for a decade. -->
      <datalist id="sidebar-categories">
        {#each CATEGORIES as value (value)}<option {value}></option>{/each}
      </datalist>

      <label class="field">
        <span class="label"><code>\ms</code> Heading</span>
        <input bind:value={heading} type="text" disabled={readOnly} placeholder="None" />
      </label>

      <label class="field grow">
        <span class="label">
          <span><code>\p</code> Body</span>
          <span class="count">
            {edit.paragraphs.length}
            {edit.paragraphs.length === 1 ? "paragraph" : "paragraphs"} · {words}
            {words === 1 ? "word" : "words"}
          </span>
        </span>
        <textarea bind:value={body} disabled={readOnly} spellcheck="true"></textarea>
      </label>

      <div class="actions">
        <button type="button" class="secondary" onclick={() => ongo(sidebar.start)}>
          Go to L{sidebar.firstLine}
        </button>
        <button type="button" class="danger" disabled={readOnly} onclick={remove}>Remove</button>
        <button type="button" class="primary" disabled={readOnly || !dirty} onclick={apply}>
          Apply to source
        </button>
      </div>
    </div>
  {:else}
    <p class="empty">
      No study sidebars in this chapter. <strong>+ New</strong> writes an
      <code>\esb</code> block at the caret.
    </p>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    flex: 1 1 auto;
    font-size: 0.78125rem;
  }

  .summary {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding-block: 0.625rem 0.375rem;
    padding-inline: 0.875rem;
    color: var(--fg3);
    font-size: 0.71875rem;
    flex: 0 0 auto;
  }

  .markers {
    font-family: var(--font-mono);
  }

  .new {
    margin-inline-start: auto;
    border: none;
    background: none;
    color: var(--acc-text);
    font: inherit;
    font-size: inherit;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
  }

  .new:hover:not(:disabled) {
    text-decoration: underline;
  }

  .new:disabled {
    color: var(--fg3);
    cursor: default;
  }

  .list {
    margin: 0;
    padding-block: 0 0.375rem;
    padding-inline: 0.5rem;
    list-style: none;
    max-block-size: 30%;
    overflow: auto;
    flex: 0 0 auto;
  }

  .entry {
    inline-size: 100%;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding-block: 0.375rem;
    padding-inline: 0.5rem;
    border: none;
    border-inline-start: 2px solid transparent;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--fg);
    font: inherit;
    font-size: 0.78125rem;
    text-align: start;
    cursor: pointer;
  }

  .entry:hover {
    background: var(--hl);
  }

  .entry.on {
    background: var(--hl);
    border-inline-start-color: var(--acc);
  }

  .ref {
    inline-size: 2.25rem;
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--acc-text);
  }

  .heading {
    flex: 1 1 auto;
    min-inline-size: 0;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cat,
  .lines {
    flex: 0 0 auto;
    color: var(--fg3);
    font-size: 0.6875rem;
  }

  .lines {
    font-family: var(--font-mono);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
    padding-block: 0.75rem;
    padding-inline: 0.875rem;
    border-block-start: 1px solid var(--line);
    flex: 1 1 auto;
    min-block-size: 0;
  }

  .where {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.71875rem;
    color: var(--fg3);
    flex: 0 0 auto;
  }

  .strong {
    color: var(--fg);
    font-weight: 500;
  }

  .anchor {
    font-family: var(--font-mono);
    color: var(--acc-text);
  }

  .span {
    margin-inline-start: auto;
    font-family: var(--font-mono);
  }

  .note {
    margin: 0;
    padding-block: 0.375rem;
    padding-inline: 0.5rem;
    border-inline-start: 2px solid var(--acc);
    background: var(--hl);
    font-size: 0.71875rem;
    color: var(--fg2);
    flex: 0 0 auto;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.1875rem;
    flex: 0 0 auto;
    min-block-size: 0;
  }

  .field.grow {
    flex: 1 1 auto;
  }

  .label {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    font-size: 0.6875rem;
    color: var(--fg3);
  }

  .count {
    font-variant-numeric: tabular-nums;
  }

  code {
    font-family: var(--font-mono);
  }

  input,
  textarea {
    inline-size: 100%;
    padding-block: 0.375rem;
    padding-inline: 0.625rem;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--fg);
    font: inherit;
    font-size: 0.78125rem;
  }

  input {
    block-size: 1.875rem;
    padding-block: 0;
  }

  textarea {
    flex: 1 1 auto;
    min-block-size: 4.5rem;
    resize: none;
    line-height: 1.5;
    /* The body is Scripture commentary and may be in any script. */
    font-family: var(--font-content);
  }

  input:disabled,
  textarea:disabled {
    opacity: 0.6;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    flex: 0 0 auto;
  }

  .secondary,
  .danger,
  .primary {
    block-size: 1.75rem;
    padding-inline: 0.625rem;
    border-radius: var(--radius);
    font: inherit;
    font-size: 0.71875rem;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
  }

  .secondary {
    border: 1px solid var(--line);
    background: none;
    color: var(--fg);
  }

  .danger {
    border: none;
    background: none;
    color: var(--err);
  }

  .primary {
    margin-inline-start: auto;
    border: none;
    background: var(--acc-fill);
    color: var(--acc-ink);
    font-weight: 600;
    font-size: 0.75rem;
    padding-inline: 0.6875rem;
  }

  .secondary:hover:not(:disabled) {
    border-color: var(--acc);
  }

  .danger:hover:not(:disabled) {
    text-decoration: underline;
  }

  .primary:disabled,
  .danger:disabled,
  .secondary:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .empty {
    margin: 0;
    padding-block: 1rem;
    padding-inline: 0.875rem;
    color: var(--fg3);
    font-size: 0.75rem;
  }
</style>
