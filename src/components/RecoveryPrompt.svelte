<script lang="ts">
  /**
   * The recovery offer (FILE-FIDELITY §4) — P4.2.
   *
   * "PID dead → a crash occurred. Compare the snapshot against disk; if they
   * differ, offer recovery with a summary: *'Unsaved changes from Tuesday 14:12
   * were found (37 lines differ)'*. Recovery is always a choice, never
   * automatic."
   *
   * A choice with no default action, which is why neither button is the primary
   * one and neither is focused first. Restoring is destructive to what is on
   * disk; discarding is destructive to the snapshot. The person is the only one
   * who knows which of the two they want, and pressing Enter without reading is
   * not an answer.
   */

  import type { Recovery } from "../lib/documentService";

  interface Props {
    /** Restores the snapshot into the editor. */
    onrestore: (recovery: Recovery) => void;
    /** Throws it away, leaving the file as it is on disk. */
    ondiscard: () => void;
  }

  let { onrestore, ondiscard }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let offer = $state<Recovery | null>(null);

  export function ask(recovery: Recovery): void {
    offer = recovery;
    dialog?.showModal();
  }

  /**
   * "Tuesday 14:12", as §4 writes it.
   *
   * A weekday and a time for anything within the last week, which is how
   * someone thinks about work they were doing recently. A full date beyond
   * that, because "Tuesday" stops meaning a particular Tuesday.
   */
  function when(taken: number): string {
    const at = new Date(taken);
    const within = Date.now() - taken < 6 * 24 * 60 * 60 * 1000;

    return at.toLocaleString(undefined, {
      weekday: within ? "long" : undefined,
      day: within ? undefined : "numeric",
      month: within ? undefined : "long",
      year: within ? undefined : "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function close(): void {
    dialog?.close();
    offer = null;
  }
</script>

<dialog bind:this={dialog} aria-label="Recover unsaved changes">
  {#if offer}
    <h2>Unsaved changes were found</h2>
    <p>
      Unsaved changes from {when(offer.taken_at)} were found
      ({offer.lines_differing}
      {offer.lines_differing === 1 ? "line differs" : "lines differ"}
      from the file on disk).
    </p>
    <p class="note">
      Easy USFM did not shut down cleanly last time. Restoring replaces what is
      in the file with these changes — nothing is written until you save.
    </p>

    <div class="actions">
      <button
        type="button"
        onclick={() => {
          ondiscard();
          close();
        }}>Discard them</button
      >
      <button
        type="button"
        onclick={() => {
          if (offer) onrestore(offer);
          close();
        }}>Restore them</button
      >
    </div>
  {/if}
</dialog>

<style>
  dialog {
    inline-size: min(28rem, 90vw);
    padding: 1rem 1.25rem 1.25rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 40%);
  }

  h2 {
    margin-block: 0 0.6rem;
    font-size: 1.05rem;
  }

  p {
    margin-block: 0 0.6rem;
    line-height: 1.5;
  }

  .note {
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-block-start: 1.1rem;
  }

  /* Neither is primary. Both are destructive to something, and the person is
     the only one who knows which. */
  .actions button {
    padding: 0.3rem 0.9rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-sunken);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }

  .actions button:hover {
    border-color: var(--accent);
  }
</style>
