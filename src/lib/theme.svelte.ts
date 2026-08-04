/**
 * Light, dark, and system themes (PRODUCT §3).
 *
 * "System" is a real third choice rather than a default, because it means
 * *follow the platform, including when it changes* — someone whose desktop
 * switches at sunset expects the editor to come with it, without reopening it.
 *
 * The resolved theme is written to `data-theme` on the root element. CSS keys
 * off that attribute and off `prefers-color-scheme`, so styling stays in the
 * stylesheet and nothing injects a style element at runtime (SECURITY §5).
 *
 * The `.svelte.ts` extension is load-bearing: runes are compiled, not
 * imported, so `$state` in a plain `.ts` file is an undefined identifier that
 * throws at import time and takes the whole application down with it — with no
 * console error, because the module never finishes evaluating.
 */

import { read, write } from "./settings";

export type Theme = "light" | "dark" | "system";
export type Resolved = "light" | "dark";

const KEY = "theme";
const THEMES: Theme[] = ["light", "dark", "system"];

const isTheme = (value: unknown): value is Theme =>
  typeof value === "string" && (THEMES as string[]).includes(value);

function systemPrefersDark(): boolean {
  return (
    typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches
  );
}

export function resolve(theme: Theme): Resolved {
  if (theme === "system") return systemPrefersDark() ? "dark" : "light";
  return theme;
}

/**
 * The theme, as reactive state.
 *
 * A class rather than a store so the `$state` rune can be used directly and
 * components read `theme.current` without a subscription to forget to cancel.
 */
class ThemeState {
  /** What the user chose. */
  current = $state<Theme>(read<Theme>(KEY, "system", isTheme));

  /** What that means right now. */
  resolved = $state<Resolved>(resolve(read<Theme>(KEY, "system", isTheme)));

  #media: MediaQueryList | null = null;

  constructor() {
    this.apply();

    // Following the system means following it as it changes, not only as it
    // was at startup.
    if (typeof matchMedia === "function") {
      this.#media = matchMedia("(prefers-color-scheme: dark)");
      this.#media.addEventListener("change", () => {
        if (this.current === "system") this.apply();
      });
    }
  }

  set(theme: Theme): void {
    this.current = theme;
    write(KEY, theme);
    this.apply();
  }

  private apply(): void {
    this.resolved = resolve(this.current);
    document.documentElement.dataset["theme"] = this.resolved;
  }
}

export const theme = new ThemeState();
export const themes = THEMES;
