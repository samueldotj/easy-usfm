<script lang="ts">
  /**
   * "This document uses a script your system has no font for."
   *
   * UNICODE §7 asks for a one-time, non-modal notice naming the script and
   * linking to a download. Non-modal because the document is still perfectly
   * editable — the text is intact, only its rendering is degraded, and a
   * dialog would stop someone from doing work they can do.
   *
   * Two severities, because the two situations are not comparable. Text
   * rendering as boxes cannot be read at all. Text rendering in a substitute
   * face is legible and merely not what was intended, and saying that as
   * loudly would train people to dismiss the notice that matters.
   */

  import type { FontReport } from "../lib/fonts.svelte";

  interface Props {
    notices: FontReport[];
    ondismiss: () => void;
  }

  let { notices, ondismiss }: Props = $props();

  const unreadable = $derived(notices.filter((entry) => entry.tofu));
  const substituted = $derived(notices.filter((entry) => !entry.tofu && entry.missing));

  const names = (entries: FontReport[]) =>
    entries.map((entry) => entry.script.name).join(", ");

  /** Where Noto actually lives. Named per script so the link lands on it. */
  const download = (entry: FontReport) =>
    `https://fonts.google.com/noto/specimen/${entry.script.font.replaceAll(" ", "+")}`;
</script>

{#if notices.length > 0}
  <!-- `status` rather than `alert`: this is worth reading, not worth
       interrupting. An alert role preempts whatever a screen reader is
       currently saying, which for someone mid-sentence in a verse is the
       wrong trade for a font. -->
  <aside class="notice" role="status" aria-label="Font notice">
    <div class="body">
      {#if unreadable.length > 0}
        <p class="tofu">
          <strong>{names(unreadable)}</strong>
          {unreadable.length === 1 ? "has" : "have"} no font on this system, so
          {unreadable.length === 1 ? "it" : "they"} will show as empty boxes. The
          text itself is unharmed and will save correctly.
        </p>
      {/if}

      {#if substituted.length > 0}
        <p>
          <strong>{names(substituted)}</strong>
          {substituted.length === 1 ? "is" : "are"} rendering in a substitute font.
          Installing the recommended one will make
          {substituted.length === 1 ? "it" : "them"} easier to read.
        </p>
      {/if}

      <p class="links">
        {#each notices as entry (entry.script.name)}
          <a href={download(entry)} target="_blank" rel="noreferrer noopener">
            {entry.script.font}
          </a>
        {/each}
      </p>
    </div>

    <button type="button" onclick={ondismiss} aria-label="Dismiss font notice">✕</button>
  </aside>
{/if}

<style>
  .notice {
    flex: 0 0 auto;
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding-block: 0.5rem;
    padding-inline: 0.75rem;
    background: color-mix(in srgb, var(--severity-warning) 14%, var(--surface));
    border-block-end: 1px solid var(--border);
    font-size: 0.8125rem;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  p {
    margin: 0;
  }

  .tofu strong {
    color: var(--severity-error);
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  a {
    color: var(--accent);
  }

  button {
    margin-inline-start: auto;
    font: inherit;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.1rem 0.3rem;
  }

  button:hover {
    color: var(--text);
  }
</style>
