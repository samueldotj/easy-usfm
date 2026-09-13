<script lang="ts">
  /**
   * The menu bar, for the browser build only.
   *
   * On the desktop the native menu is three pixels above this one and is the
   * real thing (PRODUCT §4) — a second bar of the same words underneath it
   * would be a bug that looked like a feature. In a browser there is no native
   * menu at all, and until now the whole of File lived in three buttons.
   *
   * # It is the palette's registry, in a different shape
   *
   * Every item here is a `PaletteCommand`, grouped by the category it already
   * carries. So a command cannot exist in the palette and be missing from the
   * menu, and neither can drift from what the shortcut does — all three
   * dispatch one id. The categories are the ones the registry actually has
   * rather than the six words the mock-up drew, because a menu named Tools
   * with nothing in it is worse than no menu named Tools.
   */

  import { PALETTE, isMac, keysFor, type PaletteCommand } from "../lib/commands";

  interface Props {
    /** Runs a command by its id — the same id the native menu emits. */
    onrun: (id: string) => void;
  }

  let { onrun }: Props = $props();

  const mac = isMac();

  /** The categories, in the order the registry lists them. */
  const groups = $derived.by(() => {
    const found = new Map<string, PaletteCommand[]>();
    for (const command of PALETTE) {
      const list = found.get(command.category) ?? [];
      list.push(command);
      found.set(command.category, list);
    }
    return [...found.entries()].map(([name, commands]) => ({ name, commands }));
  });

  /** Which menu is open, by name. */
  let open = $state<string | null>(null);
  let bar = $state<HTMLElement>();

  function toggle(name: string): void {
    open = open === name ? null : name;
  }

  /**
   * Once one menu is open, pointing at another opens it.
   *
   * The behaviour every menu bar has had for thirty years, and its absence is
   * the thing that makes a hand-built one feel wrong before anyone can say
   * why.
   */
  function hover(name: string): void {
    if (open !== null) open = name;
  }

  function run(id: string): void {
    open = null;
    onrun(id);
  }

  /**
   * The keyboard, on the buttons rather than on the bar around them.
   *
   * A container listening for keys is a non-interactive element with a
   * keyboard handler, which is both a lint failure and the thing the lint is
   * about: the keys only ever arrive because something focusable inside it has
   * focus, so that is where the handler belongs.
   */
  function onKeyDown(event: KeyboardEvent, name: string): void {
    const titles = [...(bar?.querySelectorAll<HTMLElement>(".title") ?? [])];
    const at = groups.findIndex((group) => group.name === name);

    switch (event.key) {
      case "Escape":
        if (open === null) return;
        open = null;
        titles[at]?.focus();
        break;

      case "ArrowLeft":
      case "ArrowRight": {
        const step = event.key === "ArrowRight" ? 1 : -1;
        const next = groups[(at + step + groups.length) % groups.length];
        if (!next) return;
        if (open !== null) open = next.name;
        titles[(at + step + titles.length) % titles.length]?.focus();
        break;
      }

      case "ArrowDown":
      case "ArrowUp": {
        if (open === null) {
          open = name;
          // Focus lands on the first item once it exists.
          queueMicrotask(() => bar?.querySelector<HTMLElement>(".item")?.focus());
          break;
        }
        const items = [...(bar?.querySelectorAll<HTMLElement>(".item") ?? [])];
        if (items.length === 0) return;
        const on = items.findIndex((item) => item === document.activeElement);
        const step = event.key === "ArrowDown" ? 1 : -1;
        items[(on + step + items.length) % items.length]?.focus();
        break;
      }

      default:
        return;
    }

    event.preventDefault();
  }

  /** A click anywhere else closes it, which a menu has to do. */
  $effect(() => {
    if (open === null) return;

    const dismiss = (event: MouseEvent) => {
      if (!bar?.contains(event.target as Node)) open = null;
    };
    // Capture, so a click on something that stops propagation still closes it.
    document.addEventListener("mousedown", dismiss, true);
    return () => document.removeEventListener("mousedown", dismiss, true);
  });
</script>

<nav class="bar" bind:this={bar} aria-label="Main menu">
  {#each groups as group (group.name)}
    <div class="group">
      <button
        type="button"
        class="title"
        class:on={open === group.name}
        aria-expanded={open === group.name}
        aria-haspopup="menu"
        onclick={() => toggle(group.name)}
        onmouseenter={() => hover(group.name)}
        onkeydown={(event) => onKeyDown(event, group.name)}
      >
        {group.name}
      </button>

      {#if open === group.name}
        <ul class="menu" role="menu" aria-label={group.name}>
          {#each group.commands as command (command.id)}
            <li role="none">
              <button
                type="button"
                class="item"
                role="menuitem"
                onclick={() => run(command.id)}
                onkeydown={(event) => onKeyDown(event, group.name)}
              >
                <span class="label">{command.label}</span>
                {#if command.keys}
                  <span class="keys">{keysFor(command.keys, mac)}</span>
                {/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/each}
</nav>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 0.125rem;
    font-size: 0.8125rem;
  }

  .group {
    position: relative;
  }

  .title {
    padding-block: 0.25rem;
    padding-inline: 0.5rem;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--fg2);
    font: inherit;
    cursor: pointer;
  }

  .title:hover,
  .title.on {
    background: var(--bg3);
    color: var(--fg);
  }

  .menu {
    position: absolute;
    inset-block-start: calc(100% + 4px);
    inset-inline-start: 0;
    z-index: 40;
    margin: 0;
    padding-block: 0.25rem;
    padding-inline: 0;
    list-style: none;
    min-inline-size: 15rem;
    background: var(--bg2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }

  .item {
    inline-size: 100%;
    display: flex;
    align-items: center;
    gap: 1.5rem;
    padding-block: 0.3rem;
    padding-inline: 0.75rem;
    border: none;
    background: none;
    color: var(--fg);
    font: inherit;
    font-size: 0.8125rem;
    text-align: start;
    cursor: pointer;
  }

  .item:hover,
  .item:focus-visible {
    background: var(--hl);
  }

  .label {
    flex: 1 1 auto;
  }

  .keys {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--fg3);
  }
</style>
