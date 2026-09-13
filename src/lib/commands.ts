/**
 * Everything the application can be asked to do, in one list.
 *
 * The command palette is the fourth way to reach a command — after the menu,
 * the toolbar and the shortcut — and it is the only one that scales: a menu
 * with sixty items is a menu nobody reads, and the answer everywhere else has
 * been to add another submenu. So this is a list rather than a tree, and it is
 * searched rather than navigated.
 *
 * # The ids are the menu's ids
 *
 * Not a parallel set. A command here dispatches the same identifier the native
 * menu emits and the toolbar sends, so the palette can never drift into being
 * a second implementation of Save — it is a second *route* to the first one.
 * Anything that needs a new behaviour gets an id in the menu too.
 *
 * # Four kinds of question, one field
 *
 * A leading `:` is a reference, `\` is a marker, `@` is a diagnostic, and
 * anything else is a command. Prefixes rather than a mode switch, because the
 * user is typing before they have decided which of the four they wanted, and
 * every one of these already has its own dialog for people who did.
 */

/** What the query is asking for. */
export type Mode = "commands" | "reference" | "marker" | "diagnostic";

/** One thing the palette can run. */
export interface PaletteCommand {
  /** The menu id. Dispatched exactly as the menu would. */
  id: string;
  /** The group shown in the left column. */
  category: string;
  label: string;
  /** The shortcut, in its Windows and Linux spelling. */
  keys?: string;
  /** Extra words that should match but do not appear in the label. */
  also?: string;
}

/**
 * The registry.
 *
 * Ordered by how often a command is wanted rather than alphabetically, because
 * the order here is the order an empty query shows — and an empty query is
 * what someone sees the instant they press the shortcut.
 */
export const PALETTE: readonly PaletteCommand[] = [
  { id: "save", category: "File", label: "Save", keys: "Ctrl S" },
  { id: "save-as", category: "File", label: "Save As…", keys: "Ctrl Shift S" },
  { id: "open", category: "File", label: "Open…", keys: "Ctrl O" },
  { id: "new", category: "File", label: "New", keys: "Ctrl N" },
  { id: "print", category: "File", label: "Print…", keys: "Ctrl P" },

  { id: "find", category: "Edit", label: "Find", keys: "Ctrl F" },
  { id: "replace", category: "Edit", label: "Replace", keys: "Ctrl H" },

  {
    id: "insert-footnote",
    category: "Insert",
    label: "Footnote \\f + \\fr … \\ft …",
    also: "note",
  },
  { id: "insert-xref", category: "Insert", label: "Cross reference \\x", also: "note" },
  { id: "insert-sidebar", category: "Insert", label: "Study sidebar \\esb … \\esbe", also: "note" },
  { id: "insert-chapter", category: "Insert", label: "Chapter \\c" },
  { id: "insert-verse", category: "Insert", label: "Verse \\v" },
  { id: "insert-paragraph", category: "Insert", label: "Paragraph \\p" },
  { id: "insert-section", category: "Insert", label: "Section heading \\s1", also: "title" },
  { id: "insert-parallel", category: "Insert", label: "Parallel references \\r" },
  { id: "insert-poetry", category: "Insert", label: "Poetry line \\q1" },
  { id: "insert-break", category: "Insert", label: "Blank line \\b" },
  { id: "insert-table", category: "Insert", label: "Table \\tr" },
  { id: "insert-figure", category: "Insert", label: "Image \\fig" },
  { id: "insert-bold", category: "Insert", label: "Bold \\bd" },
  { id: "insert-italic", category: "Insert", label: "Italic \\it" },

  { id: "go-to-reference", category: "Go to", label: "Reference…", keys: "Ctrl G" },
  { id: "next-diagnostic", category: "Go to", label: "Next diagnostic", keys: "F8" },
  { id: "previous-diagnostic", category: "Go to", label: "Previous diagnostic", keys: "Shift F8" },
  { id: "focus-editor", category: "Go to", label: "Editor pane", keys: "Ctrl 1" },
  { id: "focus-preview", category: "Go to", label: "Preview pane", keys: "Ctrl 2" },

  { id: "shell-workbench", category: "View", label: "Workbench layout", also: "columns" },
  { id: "shell-study", category: "View", label: "Study layout", also: "reading" },
  { id: "panes-editor", category: "View", label: "Editor only" },
  { id: "panes-split", category: "View", label: "Split" },
  { id: "panes-preview", category: "View", label: "Preview only" },
  { id: "toggle-sync", category: "View", label: "Sync scroll" },
  { id: "toggle-navigator", category: "View", label: "Navigator" },
  { id: "toggle-diagnostics", category: "View", label: "Diagnostics", keys: "Ctrl Shift M" },
  { id: "toggle-images", category: "View", label: "Show images" },
  { id: "toggle-invisibles", category: "View", label: "Show invisible characters", keys: "Ctrl Shift 8" },
  { id: "zoom-in", category: "View", label: "Zoom in", keys: "Ctrl +" },
  { id: "zoom-out", category: "View", label: "Zoom out", keys: "Ctrl −" },
  { id: "zoom-reset", category: "View", label: "Actual size", keys: "Ctrl 0" },
  { id: "theme:light", category: "View", label: "Light theme" },
  { id: "theme:dark", category: "View", label: "Dark theme" },
  { id: "theme:system", category: "View", label: "System theme" },

  { id: "marker-reference", category: "Help", label: "Marker reference", keys: "F1" },
] as const;

/**
 * What a query means, and what is left of it once the prefix is taken off.
 *
 * The prefix has to be the first character. A colon in the middle of "go to
 * 3:16" is part of the reference, not a second mode switch.
 */
export function modeOf(query: string): { mode: Mode; term: string } {
  const first = query.slice(0, 1);
  if (first === ":") return { mode: "reference", term: query.slice(1).trim() };
  if (first === "\\") return { mode: "marker", term: query.slice(1).trim() };
  if (first === "@") return { mode: "diagnostic", term: query.slice(1).trim() };
  return { mode: "commands", term: query.trim() };
}

/**
 * How well a term matches a label. Lower is better; `null` is no match.
 *
 * Three tiers, in the order they are worth: the label starts with the term,
 * a word inside it does, or the letters appear in order somewhere. The third
 * is what makes "isf" find "Insert Study sidebar", and it is ranked last
 * because on its own it matches almost everything.
 */
export function score(command: PaletteCommand, term: string): number | null {
  if (term === "") return 0;

  const needle = term.toLowerCase();
  const haystacks = [command.label, command.category, command.also ?? ""];

  let best: number | null = null;
  for (const [index, source] of haystacks.entries()) {
    const hay = source.toLowerCase();
    // The category matching counts, but for less: searching "view" should
    // bring up the View commands, under anything whose label says it.
    const penalty = index * 100;

    if (hay.startsWith(needle)) best = min(best, 0 + penalty);
    else if (wordStart(hay, needle)) best = min(best, 10 + penalty);
    else if (hay.includes(needle)) best = min(best, 20 + penalty);
    else if (subsequence(hay, needle)) best = min(best, 40 + penalty);
  }

  return best;
}

const min = (a: number | null, b: number) => (a === null ? b : Math.min(a, b));

/** Whether the term starts a word — after a space, a slash or a backslash. */
function wordStart(hay: string, needle: string): boolean {
  let at = hay.indexOf(needle);
  while (at > 0) {
    const before = hay[at - 1] ?? "";
    if (before === " " || before === "\\" || before === "/" || before === "-") return true;
    at = hay.indexOf(needle, at + 1);
  }
  return false;
}

/** Whether every letter of the term appears in order. */
function subsequence(hay: string, needle: string): boolean {
  let at = 0;
  for (const letter of needle) {
    at = hay.indexOf(letter, at);
    if (at === -1) return false;
    at += 1;
  }
  return true;
}

/**
 * The matching commands, best first.
 *
 * Stable within a score, so the registry's own ordering survives — which is
 * what keeps Save above Save As when both match equally.
 */
export function rank(
  term: string,
  commands: readonly PaletteCommand[] = PALETTE,
): PaletteCommand[] {
  return commands
    .map((command, at) => ({ command, at, score: score(command, term) }))
    .filter((entry): entry is { command: PaletteCommand; at: number; score: number } =>
      entry.score !== null,
    )
    .sort((a, b) => a.score - b.score || a.at - b.at)
    .map((entry) => entry.command);
}

/**
 * A shortcut written for the platform reading it.
 *
 * PRODUCT §7: Command on macOS wherever the table says Ctrl. Showing "Ctrl S"
 * on a Mac is worse than showing nothing, because it is a key that does not
 * work presented as one that does.
 */
export function keysFor(keys: string | undefined, mac: boolean): string | undefined {
  if (keys === undefined) return undefined;
  if (!mac) return keys;
  return keys.replace(/\bCtrl\b/g, "⌘").replace(/\bShift\b/g, "⇧").replace(/\bAlt\b/g, "⌥");
}

/** Whether this is a Mac, for the shortcut spelling only. */
export function isMac(): boolean {
  if (typeof navigator === "undefined") return false;
  // `platform` is deprecated and still the only thing every engine agrees on;
  // `userAgentData` is Chromium-only. Both are consulted and neither is
  // trusted for anything but which word to print on a key.
  const data = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData;
  const name = data?.platform ?? navigator.platform ?? "";
  return /mac/i.test(name);
}
