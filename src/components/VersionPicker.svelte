<script lang="ts">
  /**
   * The document's USFM version, in the status bar, overridable (PRODUCT §4).
   *
   * The override changes how the document is *judged*, never what it contains.
   * PRODUCT §4 is explicit that the detected version is never written into the
   * file automatically, so this writes nothing — a document looked at as 2.0
   * and then saved is byte-identical to one that was not.
   */

  import type { UsfmVersion } from "../worker/protocol";

  interface Props {
    version: UsfmVersion;
    onchange: (version: string | null) => void;
  }

  let { version, onchange }: Props = $props();

  /**
   * What can be chosen.
   *
   * Not every version the parser understands. 1.x predates anything still in
   * circulation, and offering it would be offering a way to make the editor
   * wrong about a file nobody has.
   */
  const CHOICES = ["2.0", "3.0", "3.1"];

  const value = $derived(version.overridden ? version.effective : "");

  // Says where the number came from, which is the part that is not obvious.
  // "3.0" alone cannot distinguish a file that declares it from the great
  // majority that declare nothing and are assumed to be it.
  const explanation = $derived(
    version.overridden
      ? `Judged as USFM ${version.effective} by your choice. The file is unchanged` +
          (version.declared ? ` and still declares ${version.declared}.` : ".")
      : version.declared
        ? `This file declares \\usfm ${version.declared}.`
        : `This file declares no version, so ${version.assumed} is assumed.`,
  );
</script>

<label class="picker" title={explanation}>
  <span class="prefix">USFM</span>
  <select
    aria-label="USFM version to judge this document as"
    {value}
    onchange={(event) => onchange(event.currentTarget.value || null)}
  >
    <!-- Qualified, because the number alone is the same string as the explicit
         choice below it and the two mean different things: this one follows
         the file if the file changes, and that one does not. -->
    <option value="">
      {version.declared
        ? `${version.declared} (from file)`
        : `${version.assumed} (assumed)`}
    </option>
    {#each CHOICES as choice (choice)}
      <option value={choice}>{choice}</option>
    {/each}
  </select>
  {#if version.overridden}
    <!-- Marked, because a severity that shifted for a reason the user set an
         hour ago and cannot see is indistinguishable from a bug. -->
    <span class="overridden">overridden</span>
  {/if}
</label>

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .prefix {
    color: var(--text-muted);
  }

  select {
    background: none;
    border: 1px solid transparent;
    border-radius: 3px;
    color: inherit;
    font: inherit;
    padding-inline: 0.15rem;
    cursor: pointer;
  }

  select:hover {
    border-color: var(--border);
  }

  .overridden {
    color: var(--accent);
  }
</style>
