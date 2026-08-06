/**
 * Print settings, and the `@page` rule they generate — PRODUCT §8, P3.11.
 *
 * "Settings, per document: page size (A4, or Letter under US/CA locale),
 * margins (20 mm outer / 18 mm inner), base font size (11 pt), notes placement,
 * include section headings, include introduction material, include
 * cross-references (off), chapter starts new page. These generate an `@page`
 * rule at print time."
 *
 * # Per document, and how that is stored
 *
 * Keyed by the document's path, because these are properties of the book being
 * printed rather than of the person printing: a Gospel set at 11 pt on A4 and a
 * reference volume set smaller on Letter should each keep their own answer.
 * A document with no path yet keeps its settings for the session and stops
 * there — there is no name to file them under, and inventing one would make two
 * unsaved documents share a page size.
 *
 * # Why the rule is generated rather than written in the stylesheet
 *
 * `@page` cannot read custom properties. Everything else about printing is in
 * `styles/print.css`, but the page box itself has to be a literal rule, so it
 * is built as text and installed at print time.
 *
 * Installed through a constructed stylesheet rather than a style element,
 * because the real policy is `style-src 'self'` with no `'unsafe-inline'`
 * (SECURITY §4) — a style element written from script is exactly what that
 * blocks. CSSOM is not markup and is not covered by it, which is the same
 * reason the split pane sets its size through a custom property.
 */

import { read, write } from "./settings";

/** Where notes go, given that per-page footnotes are impossible. */
export type NotesPlacement = "chapter" | "document";

export type PageSize = "a4" | "letter";

export interface PrintSettings {
  size: PageSize;
  /** Millimetres. Outer is the trimmed edge, inner the bound one. */
  marginOuter: number;
  marginInner: number;
  /** Points. */
  fontSize: number;
  notes: NotesPlacement;
  sectionHeadings: boolean;
  introduction: boolean;
  /**
   * Cross-references, off by default (PRODUCT §8).
   *
   * A reading copy is the common reason to print, and `\x` notes are apparatus
   * — useful to a translator, noise to a reader. The setting is there because
   * the translator case is real, not because the default is uncertain.
   */
  crossReferences: boolean;
  chapterStartsPage: boolean;
}

/** Page boxes in millimetres, which is what `@page size` takes either way. */
const PAGES: Record<PageSize, { inline: number; block: number }> = {
  a4: { inline: 210, block: 297 },
  letter: { inline: 215.9, block: 279.4 },
};

/**
 * A4 everywhere except where Letter is the paper people actually have.
 *
 * The region rather than the language: `en-GB` is A4 and `es-US` is Letter, so
 * reading the language would get both wrong. Falls back to A4, which is the
 * majority of the world and the majority of translation work.
 */
export function defaultSize(locale: string = navigator.language): PageSize {
  const region = new Intl.Locale(locale).maximize().region;
  return region === "US" || region === "CA" || region === "PH" ? "letter" : "a4";
}

export function defaults(): PrintSettings {
  return {
    size: defaultSize(),
    marginOuter: 20,
    marginInner: 18,
    fontSize: 11,
    notes: "chapter",
    sectionHeadings: true,
    introduction: true,
    crossReferences: false,
    chapterStartsPage: true,
  };
}

/**
 * The `@page` rule and the settings that have to reach the cascade.
 *
 * One string rather than several rules, because it is installed and replaced
 * atomically — a partially applied set of print settings is a page nobody
 * asked for.
 *
 * `margin` takes the outer value on three sides and the inner one on the
 * binding edge, which `@page :left` and `:right` distinguish. A single-sided
 * document prints the same either way, and a double-sided one gets its gutter
 * on the correct side of each leaf without the reader configuring anything.
 */
export function pageRule(settings: PrintSettings): string {
  const page = PAGES[settings.size];
  const outer = `${settings.marginOuter}mm`;
  const inner = `${settings.marginInner}mm`;

  return `
@page {
  size: ${page.inline}mm ${page.block}mm;
  margin: ${outer};
}
@page :left {
  margin-right: ${inner};
}
@page :right {
  margin-left: ${inner};
}
@media print {
  :root {
    --print-font-size: ${settings.fontSize}pt;
  }
}
`.trim();
}

/**
 * The classes the print stylesheet keys its optional parts off.
 *
 * On the root element rather than passed into every component: what a section
 * heading looks like is the stylesheet's business, and whether it is printed at
 * all is one switch that several rules read.
 */
export function printClasses(settings: PrintSettings): string[] {
  const classes: string[] = [`print-notes-${settings.notes}`];
  if (!settings.sectionHeadings) classes.push("print-no-headings");
  if (!settings.introduction) classes.push("print-no-intro");
  if (!settings.crossReferences) classes.push("print-no-xrefs");
  // Named for what is true rather than for the default, so the stylesheet
  // reads the same way round as the setting does.
  if (settings.chapterStartsPage) classes.push("print-chapter-page");
  return classes;
}

const KEY = "print";

function isSettings(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Reads a stored set, filling anything missing or malformed from defaults. */
export function restore(stored: unknown): PrintSettings {
  const base = defaults();
  if (!isSettings(stored)) return base;

  // Field by field rather than a spread, so a stored file with an extra or
  // mistyped key cannot put a value the rest of this module does not expect
  // into a generated CSS rule.
  return {
    size: stored.size === "letter" || stored.size === "a4" ? stored.size : base.size,
    marginOuter: millimetres(stored.marginOuter, base.marginOuter),
    marginInner: millimetres(stored.marginInner, base.marginInner),
    fontSize: points(stored.fontSize, base.fontSize),
    notes: stored.notes === "document" ? "document" : "chapter",
    sectionHeadings: boolean(stored.sectionHeadings, base.sectionHeadings),
    introduction: boolean(stored.introduction, base.introduction),
    crossReferences: boolean(stored.crossReferences, base.crossReferences),
    chapterStartsPage: boolean(stored.chapterStartsPage, base.chapterStartsPage),
  };
}

/**
 * Clamped rather than validated. A margin wider than the page is a blank
 * sheet, and a stored value is not user input worth arguing with.
 */
function millimetres(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.min(Math.max(value, 0), 60)
    : fallback;
}

function points(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value)
    ? Math.min(Math.max(value, 6), 24)
    : fallback;
}

function boolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

class Print {
  current = $state<PrintSettings>(defaults());

  /** Which document the current settings belong to, by path. */
  #path: string | null = null;

  /** The generated rule, kept so it can be replaced rather than accumulated. */
  #sheet: CSSStyleSheet | null = null;

  /**
   * Loads the settings for a document, or the defaults for one with no path.
   *
   * Called whenever the document changes. An unsaved document gets defaults
   * rather than the previous document's settings, so page size does not
   * silently follow the reader from one book to the next.
   */
  load(path: string | null): void {
    if (path === this.#path) return;
    this.#path = path;

    this.current = path === null ? defaults() : restore(read(key(path), null, unknown));
  }

  /** Changes one setting and stores the result, where there is somewhere to. */
  set<K extends keyof PrintSettings>(field: K, value: PrintSettings[K]): void {
    this.current = { ...this.current, [field]: value };
    if (this.#path !== null) write(key(this.#path), this.current);
  }

  /**
   * Installs the generated rule, replacing any previous one.
   *
   * Called before printing rather than on every change: the rule only affects
   * the print rendering, and rewriting a stylesheet on each keystroke of a
   * margin field would be work nobody can see.
   */
  apply(): void {
    const rule = pageRule(this.current);

    // Constructed stylesheets where they exist, which is everywhere this ships
    // except older WebKitGTK. The fallback writes into a sheet the page already
    // owns, which is also CSSOM and also allowed by the policy.
    if (typeof CSSStyleSheet !== "undefined" && "replaceSync" in CSSStyleSheet.prototype) {
      this.#sheet ??= new CSSStyleSheet();
      this.#sheet.replaceSync(rule);
      if (!document.adoptedStyleSheets.includes(this.#sheet)) {
        document.adoptedStyleSheets = [...document.adoptedStyleSheets, this.#sheet];
      }
    }

    const root = document.documentElement;
    for (const existing of [...root.classList]) {
      if (existing.startsWith("print-")) root.classList.remove(existing);
    }
    root.classList.add(...printClasses(this.current));
  }

  /**
   * PRODUCT §8: `window.print()` on both targets.
   *
   * Tauri 2 has no native print API, the webview path is correct everywhere,
   * and it yields Save as PDF for free.
   */
  print(): void {
    this.apply();
    window.print();
  }
}

const key = (path: string) => `${KEY}.${path}`;
const unknown = (value: unknown): value is unknown => value !== undefined;

export const print = new Print();
