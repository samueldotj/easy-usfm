<script lang="ts">
  /**
   * The first-run update-check prompt (PRODUCT §11) — P6.3.
   *
   * "Opt-in on first run, with the prompt stating it is the application's only
   * network request."
   *
   * The wording is the feature. SECURITY §6 promises something the user cannot
   * check for themselves — that this is the only request this application will
   * ever make — so the prompt says it plainly, says what is sent, and offers
   * "No" as an equal choice rather than a grey afterthought beside a blue
   * "Yes". Neither button is primary, for the same reason the recovery prompt's
   * are not: the person is the only one who knows which they want.
   */

  import { updates } from "../lib/updates.svelte";
</script>

{#if updates.asking}
  <div class="ask" role="dialog" aria-label="Check for updates" aria-modal="false">
    <p>
      <strong>Check for updates?</strong>
      Easy USFM can check whether a newer version is available.
    </p>
    <p class="detail">
      This would be the only network request this application ever makes. It
      sends the version you are running and nothing else — no document, no file
      name, no identifier. Your work never leaves this machine either way.
    </p>
    <div class="actions">
      <button type="button" onclick={() => updates.answer("allowed")}>Yes, check</button>
      <button type="button" onclick={() => updates.answer("refused")}>No, never</button>
    </div>
  </div>
{/if}

<style>
  .ask {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding-block: 0.6rem;
    padding-inline: 0.9rem;
    background: var(--surface-sunken);
    border-block-end: 1px solid var(--border);
    color: var(--text);
    font-size: 0.85rem;
  }

  p {
    margin: 0;
    line-height: 1.5;
  }

  .detail {
    color: var(--text-muted);
    font-size: 0.82rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    margin-block-start: 0.25rem;
  }

  /* Neither is primary. A styled "Yes" beside a plain "No" is a nudge, and
     this is a question about somebody's network, not a conversion funnel. */
  button {
    padding-block: 0.2rem;
    padding-inline: 0.7rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface);
    color: inherit;
    font: inherit;
    font-size: inherit;
    cursor: pointer;
  }

  button:hover {
    border-color: var(--accent);
  }
</style>
