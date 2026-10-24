<script lang="ts" generics="T extends string">
  /**
   * The three-way toggle the redesign uses in four places: the theme, the pane
   * layout, the diagnostics filter and the shell.
   *
   * Radio inputs rather than buttons, because that is what this is — one
   * choice from a short set — and the browser then gives the arrow-key
   * behaviour, the group semantics and the single tab stop for free. A row of
   * buttons with `aria-pressed` looks identical and is three separate tab
   * stops that announce as three unrelated toggles.
   *
   * The inputs are off screen rather than `display: none`, which would take
   * them out of the accessibility tree and out of the tab order along with it.
   */

  interface Option {
    value: T;
    label: string;
    /** The longer form, for the tooltip. */
    title?: string;
  }

  interface Props {
    /** Distinguishes this group's radios from every other group's. */
    name: string;
    /** What the group is, for anyone who cannot see it sitting next to a label. */
    label: string;
    options: Option[];
    value: T;
    onchange: (value: T) => void;
    /** Smaller, for the ones that sit inside a panel header. */
    compact?: boolean;
  }

  let { name, label, options, value, onchange, compact = false }: Props = $props();
</script>

<div class="seg" class:compact role="group" aria-label={label}>
  {#each options as option (option.value)}
    <label class="opt" class:on={option.value === value} title={option.title ?? option.label}>
      <input
        type="radio"
        {name}
        value={option.value}
        checked={option.value === value}
        onchange={() => onchange(option.value)}
      />
      <span>{option.label}</span>
    </label>
  {/each}
</div>

<style>
  .seg {
    display: flex;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
    font-size: 0.75rem;
    flex: 0 0 auto;
  }

  .opt {
    display: flex;
    align-items: center;
    padding-block: 0.3rem;
    padding-inline: 0.625rem;
    color: var(--fg2);
    cursor: pointer;
    white-space: nowrap;
  }

  .compact .opt {
    padding-block: 0.15rem;
    padding-inline: 0.5rem;
    font-size: 0.6875rem;
  }

  .opt.on {
    background: var(--bg3);
    color: var(--fg);
    font-weight: 500;
  }

  .opt:hover:not(.on) {
    background: var(--hl);
    color: var(--fg);
  }

  /* Off screen, not gone: `display: none` would take the radio out of the
     accessibility tree and the tab order with it. */
  input {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }

  /* The ring goes on the label, since the input it belongs to is invisible. */
  .opt:has(input:focus-visible) {
    outline: 2px solid var(--acc);
    outline-offset: -2px;
  }
</style>
