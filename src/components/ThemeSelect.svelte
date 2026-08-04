<script lang="ts">
  import { theme, themes, type Theme } from "../lib/theme.svelte";

  const label: Record<Theme, string> = {
    light: "Light",
    dark: "Dark",
    system: "System",
  };
</script>

<label class="theme">
  <span>Theme</span>
  <select
    value={theme.current}
    onchange={(event) => theme.set(event.currentTarget.value as Theme)}
  >
    {#each themes as option (option)}
      <option value={option}>{label[option]}</option>
    {/each}
  </select>
</label>

<style>
  .theme {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.8125rem;
    color: var(--text-muted);
  }

  select {
    font: inherit;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding-block: 0.15rem;
    padding-inline: 0.35rem;
  }
</style>
