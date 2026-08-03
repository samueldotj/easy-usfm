/**
 * Settings that survive a restart.
 *
 * `localStorage` for now. PRODUCT §12 lists what eventually lives here — font,
 * per-script size, line height, theme, Backspace behaviour, suppressed
 * diagnostic codes — and some of that has to be readable by the shell rather
 * than only by the webview, so this becomes a real store later. Reading and
 * writing through one place is what makes that a change in one file.
 *
 * Every read is total: a corrupt or absent value falls back rather than
 * throwing. Settings are a convenience, and failing to start because one is
 * malformed would be a poor trade.
 */

const PREFIX = "easy-usfm.";

export function read<T>(key: string, fallback: T, valid: (value: unknown) => value is T): T {
  try {
    const raw = localStorage.getItem(PREFIX + key);
    if (raw === null) return fallback;

    const parsed: unknown = JSON.parse(raw);
    return valid(parsed) ? parsed : fallback;
  } catch {
    return fallback;
  }
}

export function write(key: string, value: unknown): void {
  try {
    localStorage.setItem(PREFIX + key, JSON.stringify(value));
  } catch {
    // Private browsing, a full quota, or a webview without storage. None of
    // them are worth interrupting the user over; the setting simply does not
    // persist.
  }
}

export const isNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);
