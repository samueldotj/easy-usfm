<script lang="ts">
  /**
   * The insert toolbar.
   *
   * Icons rather than words, because there are nine of them and a row of nine
   * words is a sentence nobody reads. Every one carries its meaning in three
   * places: the icon, a `title` for the hover tooltip, and an `aria-label` for
   * anyone who will never see either.
   *
   * # The icons are drawn here
   *
   * Inline SVG, not an icon font and not sprites fetched at runtime. A font
   * would be a network request the offline build must not make (PRODUCT §12),
   * and a fetched sprite is blocked by the policy outright (SECURITY §4). They
   * are also `currentColor` throughout, so they follow the theme without a
   * second set for dark mode.
   *
   * `aria-hidden` on every icon: the button already has a label, and a screen
   * reader announcing "Bold graphic, Bold" is the same thing twice.
   *
   * # On both platforms
   *
   * Unlike the file commands, this is not something a native menu shows at a
   * glance. Bold and Italic are toolbar buttons in every editor a translator
   * has used, and putting them only in a menu on the desktop would make the
   * desktop build the awkward one. The menu has them too — the same commands,
   * by the same ids.
   */

  import { COMMANDS } from "../lib/insert";

  interface Props {
    /** Runs a command by id. The ids are the menu's ids. */
    oninsert: (id: string) => void;
    /** Nothing can be inserted into a document another window holds. */
    disabled?: boolean;
  }

  let { oninsert, disabled = false }: Props = $props();
</script>

<div class="insert" role="toolbar" aria-label="Insert">
  {#each COMMANDS as command (command.id)}
    <button
      type="button"
      title={command.help}
      aria-label={command.label}
      {disabled}
      onclick={() => oninsert(command.id)}
    >
      <!--
        24×24 throughout, stroked rather than filled, so they weigh the same
        beside each other and beside the interface's text.
      -->
      <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        {#if command.id === "insert-chapter"}
          <!-- A capital C: the marker this writes. -->
          <text x="12" y="17" class="glyph">C</text>
        {:else if command.id === "insert-verse"}
          <text x="12" y="17" class="glyph">V</text>
        {:else if command.id === "insert-bold"}
          <text x="12" y="17" class="glyph bold">B</text>
        {:else if command.id === "insert-italic"}
          <text x="12" y="17" class="glyph italic">I</text>
        {:else if command.id === "insert-paragraph"}
          <!-- Three full lines: a paragraph of prose. -->
          <path d="M4 6h16M4 12h16M4 18h11" />
        {:else if command.id === "insert-break"}
          <!-- Two blocks with a gap: the blank line between stanzas. -->
          <path d="M4 5h16M4 9h16M4 19h16M4 15h16" />
          <path d="M4 12h5M15 12h5" class="faint" />
        {:else if command.id === "insert-poetry"}
          <!-- Indented short lines: poetry, set in from the margin. -->
          <path d="M4 6h16M8 12h12M8 18h9" />
        {:else if command.id === "insert-table"}
          <path d="M4 5h16v14H4zM4 10h16M10 5v14" />
        {:else if command.id === "insert-figure"}
          <!-- A frame with a horizon and a sun: a picture. -->
          <path d="M4 5h16v14H4z" />
          <path d="M4 15l4-4 3 3 4-5 5 6" />
          <circle cx="9" cy="9" r="1.4" />
        {/if}
      </svg>
    </button>
  {/each}
</div>

<style>
  .insert {
    display: flex;
    flex-wrap: wrap;
    gap: 0.15rem;
    align-items: center;
    padding-block: 0.2rem;
    padding-inline: 0.5rem;
    background: var(--surface);
    border-block-end: 1px solid var(--border);
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    inline-size: 1.85rem;
    block-size: 1.85rem;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 4px;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: var(--border);
    color: var(--text);
    background: var(--surface-sunken);
  }

  /* Visible focus, because this row is reachable by keyboard and a toolbar
     whose focus cannot be seen is one that cannot be used without a mouse. */
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    color: var(--text);
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  svg {
    inline-size: 1.1rem;
    block-size: 1.1rem;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  /* The lettered icons are type, not strokes. */
  .glyph {
    fill: currentColor;
    stroke: none;
    font-family: var(--font-ui, system-ui), sans-serif;
    font-size: 15px;
    font-weight: 600;
    text-anchor: middle;
  }

  .bold {
    font-weight: 800;
  }

  .italic {
    font-style: italic;
    font-weight: 500;
  }

  .faint {
    opacity: 0.45;
  }
</style>
