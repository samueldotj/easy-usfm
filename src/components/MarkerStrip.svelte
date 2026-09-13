<script lang="ts">
  /**
   * Study's marker strip: the markers themselves, grouped by what they do.
   *
   * The Workbench's insert row is icons, because it sits in a 38px pane header
   * and icons are what fits. Study has a full-width strip along the bottom and
   * uses it differently — the markers are written out as markers, grouped, so
   * the strip doubles as a reminder of what the format offers. Somebody
   * learning USFM reads this row; somebody who knows it clicks it.
   *
   * The groups are the ones the design draws, and they are the same five any
   * USFM primer uses. Each chip inserts through the same path the toolbar and
   * the menu use — see `insertMarker` in the application shell.
   */

  interface Props {
    /** Inserts a marker by name. */
    onmarker: (marker: string) => void;
    /** Opens the diagnostics table. */
    ondiagnostics: () => void;
    errors: number;
    warnings: number;
    disabled: boolean;
  }

  let { onmarker, ondiagnostics, errors, warnings, disabled }: Props = $props();

  const GROUPS: { name: string; markers: [string, string][] }[] = [
    {
      name: "Structure",
      markers: [
        ["c", "Chapter"],
        ["v", "Verse"],
        ["p", "Paragraph"],
        ["b", "Blank line"],
      ],
    },
    {
      name: "Headings",
      markers: [
        ["s1", "Section heading"],
        ["s2", "Subsection heading"],
        ["r", "Parallel references"],
      ],
    },
    {
      name: "Poetry",
      markers: [
        ["q1", "Poetry line, first level"],
        ["q2", "Poetry line, second level"],
      ],
    },
    {
      name: "Notes",
      markers: [
        ["f", "Footnote"],
        ["x", "Cross reference"],
        ["esb", "Study sidebar"],
      ],
    },
    {
      name: "Character",
      markers: [
        ["nd", "Name of God"],
        ["wj", "Words of Jesus"],
        ["add", "Translator's addition"],
      ],
    },
  ];

  const summary = $derived.by(() => {
    if (errors === 0 && warnings === 0) return "No problems";
    const parts: string[] = [];
    if (errors > 0) parts.push(`${errors} ${errors === 1 ? "error" : "errors"}`);
    if (warnings > 0) parts.push(`${warnings} ${warnings === 1 ? "warning" : "warnings"}`);
    return parts.join(" · ");
  });
</script>

<div class="strip" role="toolbar" aria-label="Insert marker">
  {#each GROUPS as group (group.name)}
    <span class="name">{group.name}</span>
    {#each group.markers as [marker, help] (marker)}
      <button
        type="button"
        class="chip"
        title="{help} — \{marker}"
        aria-label={help}
        {disabled}
        onclick={() => onmarker(marker)}
      >
        \{marker}
      </button>
    {/each}
  {/each}

  <button
    type="button"
    class="problems"
    class:bad={errors > 0}
    class:clean={errors === 0 && warnings === 0}
    onclick={ondiagnostics}
  >
    <span class="dot" aria-hidden="true"></span>
    {summary}
    <span class="chevron" aria-hidden="true">▴</span>
  </button>
</div>

<style>
  .strip {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding-block: 0.375rem;
    padding-inline: 1.125rem;
    border-block-start: 1px solid var(--line);
    background: var(--bg2);
    overflow-x: auto;
    flex: 0 0 auto;
  }

  .name {
    margin-inline-start: 0.5rem;
    font-size: 0.59375rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg3);
    white-space: nowrap;
    flex: 0 0 auto;
  }

  .name:first-child {
    margin-inline-start: 0;
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

  .problems {
    margin-inline-start: auto;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding-block: 0.25rem;
    padding-inline: 0.625rem;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: none;
    color: var(--fg);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
    white-space: nowrap;
    flex: 0 0 auto;
  }

  .problems:hover {
    border-color: var(--acc);
  }

  .dot {
    inline-size: 0.5rem;
    block-size: 0.5rem;
    border-radius: 50%;
    background: var(--acc);
  }

  .problems.bad .dot {
    background: var(--err);
  }

  .problems.clean .dot {
    background: none;
    border: 1.5px solid var(--fg3);
  }

  .chevron {
    color: var(--fg3);
  }
</style>
