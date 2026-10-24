<script lang="ts">
  /**
   * One field that reaches everything — Ctrl+K, or ⌘K.
   *
   * The fourth route to a command, after the menu, the toolbar and the
   * shortcut, and the only one that scales: sixty commands is a menu nobody
   * reads and a shortcut table nobody remembers.
   *
   * # Four questions, one field
   *
   * A leading `:` is a reference, `\` is a marker, `@` is a diagnostic, and
   * anything else is a command. Prefixes rather than a mode switch, because
   * the typing starts before the decision does. Each of the four still has its
   * own dialog for someone who knew which they wanted.
   *
   * # It runs the same ids as the menu
   *
   * Nothing is implemented here. Every row dispatches the identifier the
   * native menu emits, so the palette cannot drift into being a second
   * implementation of Save.
   *
   * A `<dialog>`, for the reasons Go to Reference gives: the platform's focus
   * trap, Escape handling and top-layer stacking are all things a hand-built
   * overlay gets at least one of wrong.
   */

  import { engine } from "../lib/engine.svelte";
  import { isMac, keysFor, modeOf, rank, type Mode } from "../lib/commands";
  import type { MarkerRow } from "../lib/markerHelp";
  import type { Diagnostic } from "../worker/protocol";

  interface Props {
    diagnostics: Diagnostic[];
    /** Runs a command by id. */
    onrun: (id: string) => void;
    /** Inserts a marker the user picked by name. */
    onmarker: (marker: string) => void;
    /** Resolves a reference. A message comes back when it could not. */
    onreference: (text: string) => Promise<string | null>;
    /** Jumps to a diagnostic by its index in the list. */
    ondiagnostic: (index: number) => void;
    /** Focus goes back where it came from. */
    onclose: () => void;
  }

  let { diagnostics, onrun, onmarker, onreference, ondiagnostic, onclose }: Props = $props();

  const mac = isMac();

  let dialog = $state<HTMLDialogElement>();
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLElement>();
  let query = $state("");
  let active = $state(0);
  let message = $state<string | null>(null);

  /** The marker table, fetched the first time a `\` is typed and then kept. */
  let markers = $state<MarkerRow[]>([]);
  let asked = false;

  export function open(): void {
    message = null;
    active = 0;
    dialog?.showModal();
    input?.select();
  }

  export function isOpen(): boolean {
    return dialog?.open ?? false;
  }

  const parsed = $derived(modeOf(query));

  /** Loading the marker table costs a WASM instantiation, so only on demand. */
  $effect(() => {
    if (parsed.mode !== "marker" || asked) return;
    asked = true;
    void engine.markerTable().then((rows) => (markers = rows));
  });

  /** One row, whatever the mode is. */
  interface Row {
    key: string;
    category: string;
    label: string;
    keys?: string;
    /** What Enter does. */
    run: () => void;
  }

  const rows = $derived.by<Row[]>(() => {
    const { mode, term } = parsed;

    if (mode === "marker") {
      const matching = markers
        .filter((row) => row.marker.toLowerCase().startsWith(term.toLowerCase()))
        .slice(0, 60);

      return matching.map((row) => ({
        key: `marker:${row.marker}`,
        category: row.class,
        label: `\\${row.marker}`,
        run: () => onmarker(row.marker),
      }));
    }

    if (mode === "diagnostic") {
      const needle = term.toLowerCase();
      return diagnostics
        .map((diagnostic, index) => ({ diagnostic, index }))
        .filter(
          ({ diagnostic }) =>
            needle === "" ||
            diagnostic.code.toLowerCase().includes(needle) ||
            diagnostic.message.toLowerCase().includes(needle),
        )
        .slice(0, 60)
        .map(({ diagnostic, index }) => ({
          key: `diagnostic:${index}`,
          category: diagnostic.severity,
          label: `${diagnostic.code} — ${diagnostic.message}`,
          keys: `L${diagnostic.line}`,
          run: () => ondiagnostic(index),
        }));
    }

    if (mode === "reference") {
      // One row, because the engine is the only thing that can say whether a
      // reference exists and asking it per keystroke is a round trip a list
      // does not need.
      if (term === "") return [];
      return [
        {
          key: "reference",
          category: "Go to",
          label: term,
          run: () => void go(term),
        },
      ];
    }

    return rank(term).map((command) => ({
      key: command.id,
      category: command.category,
      label: command.label,
      keys: keysFor(command.keys, mac),
      run: () => onrun(command.id),
    }));
  });

  // The highlight goes back to the top whenever the list changes underneath
  // it, so Enter never runs whatever happens to be sitting at the old index.
  $effect(() => {
    rows.length;
    if (active >= rows.length) active = 0;
  });

  async function go(text: string): Promise<void> {
    const failure = await onreference(text);
    if (failure === null) {
      dialog?.close();
      return;
    }
    // The dialog stays open with the reason: the fix is almost always one
    // character, and "that verse is in a different file" is not something the
    // user can act on by retyping.
    message = failure;
  }

  function choose(index: number): void {
    const row = rows[index];
    if (!row) return;
    // Closed first, so a command that moves focus into the editor is not
    // fighting the dialog's own focus restoration.
    if (parsed.mode !== "reference") dialog?.close();
    row.run();
  }

  function move(to: number): void {
    if (rows.length === 0) return;
    active = (to + rows.length) % rows.length;
    list?.querySelector<HTMLElement>(`[data-row="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }

  function onKeyDown(event: KeyboardEvent): void {
    switch (event.key) {
      case "ArrowDown":
        move(active + 1);
        break;
      case "ArrowUp":
        move(active - 1);
        break;
      case "Home":
        move(0);
        break;
      case "End":
        move(rows.length - 1);
        break;
      case "Enter":
        choose(active);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  const placeholder: Record<Mode, string> = {
    commands: "Type a command…",
    reference: "Chapter and verse, e.g. 3:16",
    marker: "Marker name, e.g. esb",
    diagnostic: "Code or message",
  };
</script>

<dialog bind:this={dialog} onclose={onclose} aria-label="Command palette">
  <div class="field">
    <span class="chevron" aria-hidden="true">›</span>
    <input
      bind:this={input}
      bind:value={query}
      type="text"
      autocomplete="off"
      spellcheck="false"
      placeholder={placeholder[parsed.mode]}
      aria-label="Command, marker, reference or diagnostic"
      aria-controls="palette-list"
      aria-activedescendant={rows.length > 0 ? `palette-row-${active}` : undefined}
      oninput={() => (message = null)}
      onkeydown={onKeyDown}
    />
    <span class="esc">Esc</span>
  </div>

  {#if message}
    <p class="message" role="status">{message}</p>
  {/if}

  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <ul id="palette-list" class="rows" role="listbox" aria-label="Results" bind:this={list}>
    {#each rows as row, index (row.key)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <li
        id="palette-row-{index}"
        data-row={index}
        role="option"
        aria-selected={index === active}
        class:on={index === active}
        onclick={() => choose(index)}
        onmousemove={() => (active = index)}
      >
        <span class="category">{row.category}</span>
        <span class="label">{row.label}</span>
        {#if row.keys}<span class="keys">{row.keys}</span>{/if}
      </li>
    {/each}

    {#if rows.length === 0}
      <li class="empty" role="presentation">
        {#if parsed.mode === "marker" && markers.length === 0}
          Loading markers…
        {:else if parsed.mode === "reference"}
          Type a reference.
        {:else}
          Nothing matches.
        {/if}
      </li>
    {/if}
  </ul>

  <p class="hints">
    <span><kbd>↑↓</kbd> navigate</span>
    <span><kbd>↵</kbd> run</span>
    <span><kbd>:</kbd> go to reference</span>
    <span><kbd>\</kbd> insert marker</span>
    <span><kbd>@</kbd> diagnostics</span>
  </p>
</dialog>

<style>
  dialog {
    inline-size: min(36rem, 92vw);
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg2);
    color: var(--fg);
    box-shadow: var(--shadow);
    font-size: 0.8125rem;
    /* Near the top rather than centred: the list grows downwards and a dialog
       that re-centres as it does is one whose first row moves while it is
       being read. */
    margin-block-start: 12vh;
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 35%);
  }

  .field {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding-inline: 0.875rem;
    block-size: 2.875rem;
    border-block-end: 1px solid var(--line);
  }

  .chevron {
    color: var(--acc-text);
    font-weight: 700;
  }

  input {
    flex: 1 1 auto;
    min-inline-size: 0;
    border: none;
    background: none;
    color: var(--fg);
    font: inherit;
    font-size: 0.9375rem;
  }

  input:focus {
    outline: none;
  }

  .esc {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    padding-block: 0.05rem;
    padding-inline: 0.3rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    color: var(--fg3);
  }

  .message {
    margin: 0;
    padding-block: 0.5rem;
    padding-inline: 0.875rem;
    color: var(--err);
    font-size: 0.75rem;
  }

  .rows {
    margin: 0;
    padding-block: 0.375rem;
    padding-inline: 0;
    list-style: none;
    max-block-size: 45vh;
    overflow: auto;
  }

  li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding-block: 0.4rem;
    padding-inline: 0.875rem;
    cursor: default;
  }

  li.on {
    background: var(--hl);
  }

  .category {
    inline-size: 4.5rem;
    flex: 0 0 auto;
    font-size: 0.625rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg3);
  }

  .label {
    flex: 1 1 auto;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  li.on .label {
    font-weight: 500;
  }

  .keys {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--fg3);
  }

  .empty {
    color: var(--fg3);
    font-style: italic;
  }

  .hints {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    margin: 0;
    padding-block: 0.5rem;
    padding-inline: 0.875rem;
    border-block-start: 1px solid var(--line);
    font-size: 0.6875rem;
    color: var(--fg3);
  }

  kbd {
    font-family: var(--font-mono);
    color: var(--fg2);
  }
</style>
