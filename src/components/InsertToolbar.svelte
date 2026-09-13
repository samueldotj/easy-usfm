<script lang="ts">
  /**
   * The insert row, in the editor pane's header.
   *
   * # Markers rather than icons
   *
   * This row used to be nine drawn icons. The redesign writes the markers out
   * instead — `\c`, `\v`, `\p` — and it is right for a reason the icons could
   * not answer: there is no picture of `\s1`. Every icon here was a letter or
   * a diagram standing in for a marker whose name is already two characters
   * long and already the thing being learned. The hover help stays, because
   * that is what says what each one does.
   *
   * Bold and Italic keep their letterforms, because those two genuinely have
   * pictures and every editor draws them.
   *
   * # On both platforms
   *
   * Unlike the file commands, this is not something a native menu shows at a
   * glance, and putting it only in a menu on the desktop would make the
   * desktop build the awkward one. The menu has the same commands, by the same
   * ids.
   */

  import { COMMANDS } from "../lib/insert";
  import Icon from "./Icon.svelte";

  interface Props {
    /** Runs a command by id. The ids are the menu's ids. */
    oninsert: (id: string) => void;
    /** Nothing can be inserted into a document another window holds. */
    disabled?: boolean;
    /** The two toggles on the right of the row. */
    onexpand: () => void;
    oninvisibles: () => void;
    expanded: boolean;
    invisibles: boolean;
  }

  let {
    oninsert,
    disabled = false,
    onexpand,
    oninvisibles,
    expanded,
    invisibles,
  }: Props = $props();

  /**
   * What each command writes, as the chip's face.
   *
   * Taken from the command's own help text, which already names the marker —
   * so a chip can never show `\bd` for a command that inserts `\it`.
   */
  function face(id: string, help: string): string {
    const marker = /\\([a-z]+[0-9]*)/.exec(help)?.[1];
    if (id === "insert-bold") return "B";
    if (id === "insert-italic") return "I";
    return marker ? `\\${marker}` : "?";
  }

  /** The order the design's strip uses: structure, headings, poetry, notes. */
  const ORDER = [
    "insert-chapter",
    "insert-verse",
    "insert-paragraph",
    "insert-section",
    "insert-parallel",
    "insert-poetry",
    "insert-break",
    "insert-footnote",
    "insert-xref",
    "insert-sidebar",
    "insert-table",
    "insert-figure",
  ];

  const chips = $derived(
    ORDER.map((id) => COMMANDS.find((command) => command.id === id)).filter(
      (command): command is (typeof COMMANDS)[number] => command !== undefined,
    ),
  );

  const styles = $derived(
    COMMANDS.filter((command) => command.id === "insert-bold" || command.id === "insert-italic"),
  );
</script>

<div class="insert" role="toolbar" aria-label="Insert">
  <!--
    The markers scroll and the two toggles do not. In a narrow editor pane
    twelve chips do not fit, and letting the whole row scroll pushes the
    toggles off the end -- controls that exist but cannot be reached without
    knowing to scroll sideways for them.
  -->
  <div class="markers">
    <span class="label">Insert</span>

  {#each chips as command (command.id)}
    <button
      type="button"
      class="chip"
      title={command.help}
      aria-label={command.label}
      {disabled}
      onclick={() => oninsert(command.id)}
    >
      {face(command.id, command.help)}
    </button>
  {/each}

  <span class="rule" aria-hidden="true"></span>

  {#each styles as command (command.id)}
    <button
      type="button"
      class="chip letter"
      class:bold={command.id === "insert-bold"}
      class:italic={command.id === "insert-italic"}
      title={command.help}
      aria-label={command.label}
      {disabled}
      onclick={() => oninsert(command.id)}
    >
      {face(command.id, command.help)}
    </button>
  {/each}
  </div>

  <span class="tools">
    <button
      type="button"
      class="tool"
      class:on={expanded}
      aria-pressed={expanded}
      title={expanded ? "Show the preview again" : "Editor only"}
      onclick={onexpand}
    >
      <Icon name={expanded ? "split" : "editor"} />
      <span class="visually-hidden">{expanded ? "Show the preview again" : "Editor only"}</span>
    </button>

    <button
      type="button"
      class="tool"
      class:on={invisibles}
      aria-pressed={invisibles}
      title="Show invisible characters"
      onclick={oninvisibles}
    >
      <Icon name="wrap" />
      <span class="visually-hidden">Show invisible characters</span>
    </button>
  </span>
</div>

<style>
  .insert {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    block-size: 2.375rem;
    padding-inline: 0.625rem;
    border-block-end: 1px solid var(--line2);
    flex: 0 0 auto;
  }

  .markers {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex: 1 1 auto;
    min-inline-size: 0;
    overflow-x: auto;
    /* The scrollbar would double the row's height for a row that is 38px by
       design; the overflow is discoverable by dragging or by a wheel. */
    scrollbar-width: none;
  }

  .markers::-webkit-scrollbar {
    display: none;
  }

  .label {
    margin-inline-end: 0.375rem;
    font-size: 0.65625rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg3);
    flex: 0 0 auto;
  }

  .chip {
    padding-block: 0.1875rem;
    padding-inline: 0.4375rem;
    border: none;
    border-radius: var(--radius-sm);
    background: var(--bg3);
    color: var(--mk);
    font-family: var(--font-mono);
    font-size: 0.75rem;
    cursor: pointer;
    flex: 0 0 auto;
  }

  .chip:hover:not(:disabled) {
    background: var(--hl);
    color: var(--fg);
  }

  .chip:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .letter {
    padding-inline: 0.5rem;
    font-family: var(--font-ui);
    color: var(--fg);
  }

  .bold {
    font-weight: 700;
  }

  .italic {
    font-style: italic;
  }

  .rule {
    inline-size: 1px;
    block-size: 1.125rem;
    background: var(--line);
    margin-inline: 0.375rem;
    flex: 0 0 auto;
  }

  .tools {
    display: flex;
    gap: 0.25rem;
    flex: 0 0 auto;
  }

  .tool {
    inline-size: 1.5rem;
    block-size: 1.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--fg3);
    cursor: pointer;
    font-size: 1rem;
  }

  .tool:hover {
    background: var(--bg3);
    color: var(--fg);
  }

  .tool.on {
    color: var(--acc-text);
  }
</style>
