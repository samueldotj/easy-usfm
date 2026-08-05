<script lang="ts">
  /**
   * Go to Reference — Ctrl+G on Windows and Linux, ⌘L on macOS (PRODUCT §6.4).
   *
   * A `<dialog>` rather than a hand-built overlay: the platform already
   * implements the focus trap, the Escape handling, the inert background and
   * the top-layer stacking, and every hand-rolled version of those gets at
   * least one of them wrong.
   *
   * The reference itself is never parsed here. The forms are the engine's to
   * know — including the `\vp` fallback, which needs the verse index — so this
   * takes a string and shows what comes back.
   */

  interface Props {
    /** Asks the engine. `null` message means it resolved. */
    onsubmit: (text: string) => Promise<string | null>;
    /** Focus goes back where it came from. */
    onclose: () => void;
  }

  let { onsubmit, onclose }: Props = $props();

  let dialog = $state<HTMLDialogElement>();
  let input = $state<HTMLInputElement>();
  let text = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);

  export function open(): void {
    error = null;
    dialog?.showModal();
    // Selected rather than cleared, so the last reference is both a starting
    // point and one keystroke from gone.
    input?.select();
  }

  export function isOpen(): boolean {
    return dialog?.open ?? false;
  }

  async function go(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (busy) return;

    busy = true;
    try {
      // The engine answers; a failure keeps the dialog open with the reason,
      // because the fix is almost always one character.
      error = await onsubmit(text);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      // In `finally`, because a lookup that throws would otherwise leave the
      // guard set and every later submit would be silently ignored — a dialog
      // that has stopped working while still looking like it works.
      busy = false;
    }

    if (error === null) dialog?.close();
    else input?.select();
  }
</script>

<dialog bind:this={dialog} onclose={onclose} aria-label="Go to Reference">
  <form onsubmit={go}>
    <label for="reference">Go to reference</label>
    <input
      id="reference"
      bind:this={input}
      bind:value={text}
      type="text"
      autocomplete="off"
      spellcheck="false"
      placeholder="GEN 1:1, 1:1, or 3"
      aria-describedby="reference-help"
      aria-invalid={error !== null}
      oninput={() => (error = null)}
    />

    <!-- Announced, because the input keeps focus on failure and a message
         that only appears visually is one a screen reader user never gets. -->
    <p id="reference-help" class="help" role="status">
      {error ?? "Chapter and verse, in any script's digits."}
    </p>

    <div class="buttons">
      <button type="button" onclick={() => dialog?.close()}>Cancel</button>
      <button type="submit" disabled={busy}>Go</button>
    </div>
  </form>
</dialog>

<style>
  dialog {
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    padding: 1rem;
    min-inline-size: 20rem;
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.35);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  label {
    font-size: 0.8125rem;
    color: var(--text-muted);
  }

  input {
    font: inherit;
    /* The reference may be typed in any script, so it gets the content font
       rather than the interface's (UNICODE §7). */
    font-family: var(--font-content);
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text);
  }

  input[aria-invalid="true"] {
    border-color: var(--severity-error);
  }

  .help {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--text-muted);
    /* Two lines' worth, so the dialog does not resize as messages come and
       go -- which moves the button out from under the pointer. */
    min-block-size: 2.4em;
  }

  input[aria-invalid="true"] + .help {
    color: var(--severity-error);
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  button {
    font: inherit;
    padding: 0.3rem 0.9rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-sunken);
    color: inherit;
    cursor: pointer;
  }

  button[type="submit"] {
    border-color: var(--accent);
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
</style>
