<script lang="ts">
  /**
   * The top row: who this is, how to reach a command, and how it looks.
   *
   * The redesign gives its two shells different top rows, and the difference
   * is not decoration — it is what each one is for.
   *
   * - **Workbench** keeps the menu words and hands the document's identity to
   *   the bar below it, because it has one.
   * - **Study** has no second bar, so the header carries the breadcrumb
   *   instead: book, chapter, section, verse. That trail is the navigator, for
   *   a layout whose navigator has shrunk to a column of numbers.
   *
   * Both carry the palette trigger, which is the one control that reaches
   * everything.
   */

  import { PALETTE, isMac, keysFor } from "../lib/commands";
  import { doc } from "../lib/document.svelte";
  import { layout } from "../lib/layout.svelte";
  import { isDesktop } from "../lib/shell";
  import { theme, type Theme } from "../lib/theme.svelte";
  import Icon from "./Icon.svelte";
  import MenuBar from "./MenuBar.svelte";
  import Segmented from "./Segmented.svelte";

  interface Props {
    /** Runs a command by id — the ids the native menu emits. */
    onrun: (id: string) => void;
    /** Opens the command palette. */
    onpalette: () => void;
    /** Where the caret is, for the Study breadcrumb. */
    chapter?: string | null;
    section?: string | null;
    reference?: string | null;
  }

  let { onrun, onpalette, chapter = null, section = null, reference = null }: Props = $props();

  const mac = isMac();
  const desktop = isDesktop();
  const paletteKeys = $derived(keysFor("Ctrl K", mac) ?? "Ctrl K");

  /** The verse alone, since the breadcrumb has already said the chapter. */
  const verse = $derived.by(() => {
    if (!reference) return null;
    const last = reference.trim().split(/\s+/).at(-1) ?? "";
    const colon = last.indexOf(":");
    return colon === -1 ? null : last.slice(colon + 1);
  });

  const panes = [
    { value: "editor" as const, label: "Editor", title: "Editor only" },
    { value: "split" as const, label: "Split", title: "Editor and preview" },
    { value: "preview" as const, label: "Preview", title: "Preview only" },
  ];

  const themes = [
    { value: "light" as const, label: "Light" },
    { value: "dark" as const, label: "Dark" },
    { value: "system" as const, label: "Auto", title: "Follow the system" },
  ];

  /** Cycles the theme, for the Study header's single icon button. */
  function nextTheme(): void {
    const order: Theme[] = ["light", "dark", "system"];
    const at = order.indexOf(theme.current);
    theme.set(order[(at + 1) % order.length] ?? "system");
  }

  const themeLabel = $derived(
    theme.current === "system" ? "Theme: follow the system" : `Theme: ${theme.current}`,
  );

  /** Whether the palette has anything to say about a command id. */
  const known = (id: string) => PALETTE.some((command) => command.id === id);
</script>

<header class="bar">
  <div class="brand">
    <span class="mark" aria-hidden="true">\</span>
    Easy USFM
  </div>

  {#if layout.shell === "workbench"}
    <!-- The desktop has a real menu three pixels above this one. -->
    {#if !desktop}
      <MenuBar {onrun} />
    {/if}
  {:else}
    <nav class="crumbs" aria-label="Where you are">
      <span class="crumb">{doc.name}</span>
      {#if chapter}
        <span class="sep" aria-hidden="true">/</span>
        <span class="crumb">Chapter {chapter}</span>
      {/if}
      {#if section}
        <span class="sep" aria-hidden="true">/</span>
        <span class="crumb here">{section}</span>
      {/if}
      {#if verse}
        <span class="sep" aria-hidden="true">/</span>
        <span class="crumb verse">v {verse}</span>
      {/if}
    </nav>
  {/if}

  <!--
    The palette, as the thing it replaces: a search field. It is a button
    because it opens a dialog rather than accepting typing here — two text
    fields that look identical and behave differently is worse than one that
    admits what it is.
  -->
  <button type="button" class="palette" onclick={onpalette}>
    <Icon name="search" />
    <span class="hint">Commands, markers, go to reference (e.g. 3:16)…</span>
    <span class="keys">{paletteKeys}</span>
  </button>

  {#if layout.shell === "workbench"}
    <Segmented
      name="theme"
      label="Theme"
      options={themes}
      value={theme.current}
      onchange={(value) => theme.set(value)}
    />
  {:else}
    <div class="tools">
      <Segmented
        name="panes-study"
        label="Panes"
        options={panes}
        value={layout.panes}
        onchange={(value) => layout.setPanes(value)}
        compact
      />

      <button
        type="button"
        class="tool"
        class:on={layout.sync}
        aria-pressed={layout.sync}
        title={layout.sync ? "Sync scroll on" : "Sync scroll off"}
        onclick={() => layout.toggleSync()}
      >
        <Icon name="sync" />
        <span class="visually-hidden">Sync scroll</span>
      </button>

      <button type="button" class="tool" title={themeLabel} onclick={nextTheme}>
        <Icon name="theme" />
        <span class="visually-hidden">{themeLabel}</span>
      </button>

      {#if known("save")}
        <button
          type="button"
          class="save"
          disabled={!doc.dirty && doc.path !== null}
          onclick={() => onrun("save")}
        >
          Save
        </button>
      {/if}
    </div>
  {/if}
</header>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 1.125rem;
    padding-block: 0.4rem;
    padding-inline: 1rem;
    background: var(--bg2);
    border-block-end: 1px solid var(--line);
    flex: 0 0 auto;
    font-size: 0.8125rem;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 600;
    font-size: 0.875rem;
    letter-spacing: -0.01em;
    flex: 0 0 auto;
  }

  /* The backslash is the application's mark: it is the character every marker
     in the format starts with. */
  .mark {
    inline-size: 1.125rem;
    block-size: 1.125rem;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--acc);
    color: var(--acc-ink);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    border-radius: var(--radius-sm);
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    min-inline-size: 0;
    overflow: hidden;
  }

  .crumb {
    padding-block: 0.2rem;
    padding-inline: 0.5rem;
    background: var(--bg3);
    border-radius: var(--radius);
    color: var(--fg2);
    white-space: nowrap;
  }

  .crumb.here {
    background: none;
    color: var(--acc-text);
    font-weight: 500;
  }

  .crumb.verse {
    background: none;
    font-family: var(--font-mono);
  }

  .sep {
    color: var(--fg3);
  }

  .palette {
    margin-inline-start: auto;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    inline-size: min(24rem, 40vw);
    block-size: 1.75rem;
    padding-inline: 0.625rem;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--fg3);
    font: inherit;
    font-size: 0.8125rem;
    cursor: pointer;
    flex: 0 0 auto;
  }

  .palette:hover {
    border-color: var(--acc);
    color: var(--fg2);
  }

  .hint {
    flex: 1 1 auto;
    text-align: start;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .keys {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 0.625rem;
    padding-block: 0.05rem;
    padding-inline: 0.3rem;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    flex: 0 0 auto;
  }

  .tool {
    inline-size: 1.75rem;
    block-size: 1.75rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius);
    background: none;
    color: var(--fg2);
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

  .save {
    block-size: 1.75rem;
    padding-inline: 0.75rem;
    border: none;
    border-radius: var(--radius);
    background: var(--acc-fill);
    color: var(--acc-ink);
    font: inherit;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
  }

  .save:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
