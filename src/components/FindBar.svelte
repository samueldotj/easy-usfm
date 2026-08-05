<script lang="ts">
  /**
   * Find and Replace (PRODUCT §6.4, UNICODE §4).
   *
   * The search itself is the engine's, against the normalized index — a query
   * typed on an NFC keyboard has to find NFD text that is visibly on screen,
   * which UNICODE §4 calls the most infuriating bug this class of application
   * can have. This is the bar, the navigation, and the toggle.
   */

  import type { Match } from "../worker/protocol";

  interface Props {
    /** Asks the engine. Positions only; the editor applies the change. */
    onsearch: (query: string, exact: boolean) => Promise<Match[]>;
    /** Selects a match and scrolls to it. */
    onreveal: (match: Match, focus: boolean) => void;
    /** Replaces one range with text, inserted exactly as typed. */
    onreplace: (match: Match, text: string) => void;
    /** Replaces every match in one transaction. */
    onreplaceall: (matches: Match[], text: string) => void;
    /** Escape releases focus (PRODUCT §10). */
    onclose: () => void;
  }

  let { onsearch, onreveal, onreplace, onreplaceall, onclose }: Props = $props();

  let open = $state(false);
  let showReplace = $state(false);
  let query = $state("");
  let replacement = $state("");
  let exact = $state(false);
  let matches = $state<Match[]>([]);
  let current = $state(0);
  let field = $state<HTMLInputElement>();
  let replaceField = $state<HTMLInputElement>();

  export function show(withReplace: boolean): void {
    open = true;
    showReplace = withReplace;
    // Deferred, because the field does not exist until the block renders.
    queueMicrotask(() => {
      const target = withReplace && field?.value ? replaceField : field;
      target?.focus();
      target?.select();
    });
    if (query) void search();
  }

  export function isOpen(): boolean {
    return open;
  }

  export function step(forward: boolean): void {
    if (matches.length === 0) return;
    current = (current + (forward ? 1 : -1) + matches.length) % matches.length;
    reveal(false);
  }

  function reveal(focus: boolean): void {
    const match = matches[current];
    if (match) onreveal(match, focus);
  }

  /**
   * Runs the search.
   *
   * Not debounced. The engine holds the whole document and the search is a
   * scan over it; the round trip is the cost, and a debounce would trade a
   * cost nobody notices for a delay everybody does.
   */
  async function search(): Promise<void> {
    const found = await onsearch(query, exact);
    matches = found;
    current = 0;
    if (found.length > 0) reveal(false);
  }

  function close(): void {
    open = false;
    matches = [];
    onclose();
  }

  function onKeyDown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      step(!event.shiftKey);
    }
  }

  function replaceOne(): void {
    const match = matches[current];
    if (!match) return;

    onreplace(match, replacement);
    // The document has changed under every later match, so the positions this
    // side holds are stale. Asking again is the only honest answer, and it is
    // one round trip on an action the user took deliberately.
    void search();
  }

  const summary = $derived(
    query === ""
      ? ""
      : matches.length === 0
        ? "No matches"
        : `${current + 1} of ${matches.length}`,
  );
</script>

{#if open}
  <div class="bar" role="search" aria-label="Find and replace">
    <div class="row">
      <label class="field">
        <span class="visually-hidden">Find</span>
        <input
          bind:this={field}
          bind:value={query}
          type="text"
          placeholder="Find"
          autocomplete="off"
          spellcheck="false"
          oninput={() => void search()}
          onkeydown={onKeyDown}
        />
      </label>

      <!-- Announced, because the count is the only feedback that a search
           found anything at all when the match is off screen. -->
      <span class="count" role="status">{summary}</span>

      <button type="button" onclick={() => step(false)} disabled={matches.length === 0}>
        Previous
      </button>
      <button type="button" onclick={() => step(true)} disabled={matches.length === 0}>
        Next
      </button>

      <label class="toggle">
        <input
          type="checkbox"
          bind:checked={exact}
          onchange={() => void search()}
        />
        <!-- UNICODE §4 names this exactly, and off by default. It is the only
             way to find out which spelling a file actually uses, which the
             normalized default deliberately cannot tell you. -->
        <span title="Compare bytes rather than characters, so the two Unicode spellings of a word are told apart">
          Match exact byte sequence
        </span>
      </label>

      <button type="button" class="close" onclick={close} aria-label="Close find">✕</button>
    </div>

    {#if showReplace}
      <div class="row">
        <label class="field">
          <span class="visually-hidden">Replace with</span>
          <input
            bind:this={replaceField}
            bind:value={replacement}
            type="text"
            placeholder="Replace with"
            autocomplete="off"
            spellcheck="false"
            onkeydown={onKeyDown}
          />
        </label>

        <button type="button" onclick={replaceOne} disabled={matches.length === 0}>
          Replace
        </button>
        <button
          type="button"
          onclick={() => {
            onreplaceall(matches, replacement);
            void search();
          }}
          disabled={matches.length === 0}
        >
          Replace all
        </button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .bar {
    flex: 0 0 auto;
    border-block-start: 1px solid var(--border);
    background: var(--surface-sunken);
    padding-block: 0.35rem;
    padding-inline: 0.6rem;
    font-size: 0.8125rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .field {
    flex: 0 1 18rem;
  }

  input[type="text"] {
    inline-size: 100%;
    font: inherit;
    /* The query may be in any script (UNICODE §7). */
    font-family: var(--font-content);
    padding: 0.2rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--surface);
    color: var(--text);
  }

  .count {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    min-inline-size: 7ch;
  }

  button {
    font: inherit;
    padding: 0.2rem 0.6rem;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--surface);
    color: inherit;
    cursor: pointer;
  }

  button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .close {
    margin-inline-start: auto;
    border-color: transparent;
    background: none;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--text-muted);
    cursor: pointer;
  }

  .visually-hidden {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
