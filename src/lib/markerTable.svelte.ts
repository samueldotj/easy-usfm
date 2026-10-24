/**
 * The marker table, fetched once for whoever asks first.
 *
 * Three places want it now — the reference page, the command palette and the
 * inspector's marker tab — and getting it costs a WASM instantiation. Three
 * copies of that is three copies of the module in memory and three chances to
 * ask for it while the first is still in flight.
 *
 * Not fetched at startup. It is a constant, but it is a constant nobody needs
 * until they open one of those three things, and paying for it during the
 * first paint is paying for it in the one place the user is watching.
 */

import { engine } from "./engine.svelte";
import { helpTable, type MarkerHelp } from "./markerHelp";

class MarkerTable {
  /** Empty until the first request lands. */
  rows = $state<MarkerHelp[]>([]);
  /** Whether a fetch is in flight, so a panel can say "loading" honestly. */
  loading = $state(false);

  /** The one request, shared by every caller that arrives while it is open. */
  #inflight: Promise<MarkerHelp[]> | null = null;

  async load(): Promise<MarkerHelp[]> {
    if (this.rows.length > 0) return this.rows;
    if (this.#inflight) return this.#inflight;

    this.loading = true;
    this.#inflight = engine
      .markerTable()
      .then((raw) => {
        this.rows = helpTable(raw);
        return this.rows;
      })
      .finally(() => {
        this.loading = false;
        // Cleared either way: a failed fetch should be retried by the next
        // caller rather than remembered as a permanent empty answer.
        this.#inflight = null;
      });

    return this.#inflight;
  }

  /** One marker, once the table is here. */
  find(marker: string): MarkerHelp | undefined {
    return this.rows.find((row) => row.marker === marker);
  }
}

export const markerTable = new MarkerTable();
