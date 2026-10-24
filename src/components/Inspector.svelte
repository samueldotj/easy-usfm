<script lang="ts">
  /**
   * The Workbench's fourth column: whatever the caret is in.
   *
   * Three tabs, because there are three kinds of thing worth inspecting and
   * they are wanted at different moments — sidebars while building a study
   * edition, notes while checking apparatus, the marker while learning the
   * format. Tabs rather than three stacked panels: the column is 292px wide
   * and a stack would give each of them a third of it.
   *
   * The tab is remembered (`layout.inspector`), because which of the three
   * someone is using is a property of the work they are doing rather than of
   * the document they are doing it to.
   */

  import { layout, type Inspector } from "../lib/layout.svelte";
  import type { NoteEntry } from "../lib/notes";
  import type { Sidebar } from "../lib/sidebars";
  import MarkerPanel from "./MarkerPanel.svelte";
  import NotePanel from "./NotePanel.svelte";
  import SidebarPanel from "./SidebarPanel.svelte";

  interface Props {
    sidebars: Sidebar[];
    notes: NoteEntry[];
    note: number;
    marker: string | null;
    chapter: string | null;
    newAt: string | null;
    readOnly: boolean;
    onapply: (from: number, to: number, text: string) => void;
    ongo: (offset: number) => void;
    onnewsidebar: () => void;
    onnote: (index: number) => void;
    onconvert: (note: NoteEntry) => void;
    onreference: () => void;
  }

  let {
    sidebars,
    notes,
    note,
    marker,
    chapter,
    newAt,
    readOnly,
    onapply,
    ongo,
    onnewsidebar,
    onnote,
    onconvert,
    onreference,
  }: Props = $props();

  const tabs: { id: Inspector; label: string; count?: number }[] = $derived([
    { id: "sidebars", label: "Sidebars", count: sidebars.length },
    { id: "footnote", label: "Notes", count: notes.length },
    { id: "marker", label: "Marker" },
  ]);

  let strip = $state<HTMLElement>();

  /** The tab pattern: one stop, arrows within. */
  function onKeyDown(event: KeyboardEvent): void {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : null;
    if (step === null) return;

    event.preventDefault();
    const at = tabs.findIndex((tab) => tab.id === layout.inspector);
    const next = tabs[(at + step + tabs.length) % tabs.length];
    if (!next) return;

    layout.setInspector(next.id);
    strip?.querySelector<HTMLElement>(`[data-tab="${next.id}"]`)?.focus();
  }
</script>

<aside class="inspector" aria-label="Inspector">
  <div class="tabs" role="tablist" aria-label="Inspector" bind:this={strip}>
    {#each tabs as tab (tab.id)}
      <button
        type="button"
        role="tab"
        class="tab"
        class:on={layout.inspector === tab.id}
        data-tab={tab.id}
        id="inspector-tab-{tab.id}"
        aria-selected={layout.inspector === tab.id}
        aria-controls="inspector-panel"
        tabindex={layout.inspector === tab.id ? 0 : -1}
        onclick={() => layout.setInspector(tab.id)}
        onkeydown={onKeyDown}
      >
        {tab.label}
        {#if tab.count !== undefined && tab.count > 0}<span class="badge">{tab.count}</span>{/if}
      </button>
    {/each}
  </div>

  <div
    class="body"
    id="inspector-panel"
    role="tabpanel"
    aria-labelledby="inspector-tab-{layout.inspector}"
    tabindex="-1"
  >
    {#if layout.inspector === "sidebars"}
      <SidebarPanel {sidebars} {chapter} {newAt} {readOnly} {onapply} {ongo} onnew={onnewsidebar} />
    {:else if layout.inspector === "footnote"}
      <NotePanel {notes} selected={note} onselect={onnote} {ongo} {onconvert} {readOnly} />
    {:else}
      <MarkerPanel {marker} {onreference} />
    {/if}
  </div>
</aside>

<style>
  .inspector {
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    overflow: hidden;
    background: var(--bg2);
  }

  .tabs {
    display: flex;
    padding-inline: 0.375rem;
    border-block-end: 1px solid var(--line);
    flex: 0 0 auto;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 0.3125rem;
    padding-block: 0.6875rem 0.5625rem;
    padding-inline: 0.625rem;
    margin-block-end: -1px;
    border: none;
    border-block-end: 2px solid transparent;
    background: none;
    color: var(--fg2);
    font: inherit;
    font-size: 0.78125rem;
    cursor: pointer;
  }

  .tab:hover {
    color: var(--fg);
  }

  .tab.on {
    color: var(--fg);
    font-weight: 500;
    border-block-end-color: var(--acc);
  }

  .badge {
    padding-inline: 0.3125rem;
    border-radius: 0.5rem;
    background: var(--bg3);
    color: var(--fg3);
    font-size: 0.625rem;
    font-variant-numeric: tabular-nums;
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-block-size: 0;
    overflow: hidden;
  }
</style>
