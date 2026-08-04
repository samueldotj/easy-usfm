# Product

Goals, scope, interface, behaviour, and acceptance criteria.

Related: [ARCHITECTURE](ARCHITECTURE.md) · [FILE-FIDELITY](FILE-FIDELITY.md) · [UNICODE](UNICODE.md) · [SECURITY](SECURITY.md) · [ROADMAP](ROADMAP.md)

---

## 1. Purpose

An editor for individual USFM Scripture files, for desktop and the web. A USFM source editor beside a live Scripture preview, using native menus, dialogs, and keyboard conventions, working entirely offline.

**The source text is authoritative** — [ADR-003](adr/003-source-authoritative.md).

The name states the goal: editing Scripture markup without first adopting a translation-suite ecosystem.

## 2. Goals

Open a `.usfm`, `.sfm`, or compatible file; edit with syntax highlighting and marker assistance; see a live preview; get clear diagnostics; navigate by chapter and verse; print a readable copy; save without losing unsupported content; work correctly in complex scripts; work offline; and get the same interface on Windows, macOS, Linux, and the web.

## 3. Scope

**In.** One file per window. New, Open, Save, Save As. Syntax highlighting, marker autocomplete, live preview, find and replace, undo and redo, chapter and verse navigation, diagnostics with navigation, character-formatting commands, print. Light, dark, and system themes. Full Unicode and complex-script support. Recovery of unsaved work. Detection of external file changes.

**Out.** Multi-book projects, collaboration, cloud sync, translation assignments, PDF typesetting, page-layout preview, PTXprint replacement, Paratext integration, Git integration, mobile apps, right-to-left *interface* mirroring ([UNICODE §8](UNICODE.md#8-text-direction)).

---

## 4. USFM handling

Targets **USFM 3.2**. Parses 2.x and all 3.x tolerantly; authors 3.2.

**Version detection.** `\usfm 3.2` in the header is authoritative. Absent that, attributes, milestones, or `\ef`/`\esb`/`\cat` imply 3.x, while positional-parameter `\fig` implies 2.x. Ambiguous files are assumed 3.2 and diagnosed as nothing — most files in circulation carry no version marker and are valid. The detected version shows in the status bar, is user-overridable, and is **never written into the file automatically**.

**Preservation.** Unknown markers, custom `\z` markers, comments, attributes, blank lines, line endings, BOM, final-newline state, Unicode normalization form, and temporarily malformed content all survive a save unchanged. Opening and saving an unchanged file produces identical bytes. Nothing is normalized or rewritten automatically; explicit normalization commands exist and are always user-initiated. Mechanics in [FILE-FIDELITY](FILE-FIDELITY.md).

---

## 5. Interface

```text
┌────────────────────────────┬────────────────────────────┐
│       USFM Editor          │     Scripture Preview      │
├────────────────────────────┴────────────────────────────┤
│  Diagnostics (collapsible)                              │
├─────────────────────────────────────────────────────────┤
│  Status bar                                             │
└─────────────────────────────────────────────────────────┘
```

Native title bar and menus, compact toolbar, resizable divider.

**Status bar:** chapter and verse (published `\vp` number when present, canonical in parentheses); line and grapheme column; encoding; line-ending style; normalization form; USFM version; text direction; save status; error and warning counts.

No custom-drawn title bars, browser-style navigation, ribbons, or gratuitous animation. Platform builds may differ where convention requires.

---

## 6. Editor

Line numbers, word wrapping, syntax highlighting, matching-marker highlighting, marker autocomplete, diagnostic decorations, grapheme-aware cursor movement, input-method support, per-line text direction.

Typing `\` opens the marker list, ranked by validity in context, then frequency in the document, then alphabetically. Deprecated markers are greyed with their replacement shown and never ranked first.

### 6.1 Formatting emits markers

Formatting inserts USFM, never hidden rich-text state. Ctrl+B on a selection produces `\bd Selected text\bd*`.

**Nesting.** USFM requires the `+` prefix for character markers inside another character marker or a note. Commands consult the tree at the insertion point:

```typescript
function markerFor(tag: string, ctx: NodeContext): string {
  return ctx.ancestors.some(a => a.class === "character" || a.class === "note")
    ? `\\+${tag}` : `\\${tag}`;
}
```

Without this, bolding inside a footnote produces invalid USFM every time.

**Toggle.**

| Situation | Ctrl+B |
|---|---|
| Selection not inside a `bd` span | Wrap (`\bd` or `\+bd` per context) |
| Selection equals a `bd` span's content | Unwrap |
| Cursor inside a `bd` span, no selection | Unwrap the span |
| Selection partially overlaps a `bd` span | Split so the selection is unmarked |
| Selection crosses a paragraph marker | Apply per fragment; never span `\p` |

Edge whitespace is excluded from the wrap — `\bd word \bd*` puts a space inside the emphasis, a common defect in hand-edited files.

### 6.2 Verse references

`\v 1-2`, `\v 1a`, `\va`, and `\vp` all occur in real files. Duplicate detection compares **ranges**, so `\v 1-2` then `\v 2` is a genuine overlap. Sequence gaps are permitted — some translations omit verses — and reported as Information.

Go to Reference accepts `GEN 1:1`, `Gen 1.1`, `1:1`, and `3`; falls back to matching `\vp` published numbers; and accepts non-ASCII digits ([UNICODE §6](UNICODE.md#6-non-ascii-digits)). Model in [ARCHITECTURE §7](ARCHITECTURE.md#7-verse-index).

### 6.3 New document

```usfm
\id XXX - Unknown Book
\h
\toc1
\toc2
\toc3
\mt1
\c 1
\p
\v 1 
```

Cursor lands after `\id `. The book-code diagnostic fires immediately, which is the intended teaching moment. Editable in settings.

### 6.4 Keyboard

| Action | Windows / Linux | macOS |
|---|---|---|
| New / Open / Save | Ctrl+N / O / S | ⌘N / O / S |
| Save As | Ctrl+Shift+S | ⇧⌘S |
| Print | Ctrl+P | ⌘P |
| Undo | Ctrl+Z | ⌘Z |
| Redo | Ctrl+Y *and* Ctrl+Shift+Z | ⇧⌘Z |
| Find | Ctrl+F | ⌘F |
| Find Next / Previous | F3 / Shift+F3 | **⌘G** / ⇧⌘G |
| Replace | Ctrl+H | ⌥⌘F |
| Go to Reference | **Ctrl+G** | **⌘L** (alias ⌥⌘G) |
| Bold / Italic marker | Ctrl+B / Ctrl+I | ⌘B / ⌘I |
| Toggle preview | Ctrl+Alt+P | ⌥⌘P |
| Next / previous diagnostic | F8 / Shift+F8 | F8 / Shift+F8 |
| Cycle pane focus | F6 | F6 |
| Focus editor / preview | Ctrl+1 / Ctrl+2 | ⌘1 / ⌘2 |
| Toggle diagnostics panel | Ctrl+Shift+M | ⇧⌘M |

Go to Reference differs by platform deliberately: `⌘G` is Find Next by universal macOS convention and is reserved for it; `⌘L` follows Xcode and VS Code.

---

## 7. Preview

Displays semantic meaning: titles, chapter and verse numbers, paragraphs, poetry, section headings, character styles, footnotes, cross-references, lists, tables, sidebars, figures, introductions, milestones, and placeholders for custom or unsupported markers.

A **reading** preview. It does not reproduce page dimensions, columns, page breaks, running headers, or PTXprint output.

An invalid marker never blanks the preview — as much valid content as possible renders, with the malformed region shown as an inline placeholder. Unpaired milestones produce a Warning and render as a chip rather than swallowing the rest of the document.

Clicking a verse or diagnostic moves the editor cursor to the corresponding source location.

Debounced 150–250 ms after typing stops, suspended entirely during input-method composition ([UNICODE §5](UNICODE.md#5-input-method-composition)). Rendering strategy in [ARCHITECTURE §10](ARCHITECTURE.md#10-preview-rendering).

---

## 8. Print

**Ctrl+P produces a clean, readable document from the preview via print CSS. Not typesetting.**

Browsers give us `@page { size; margin }`, `break-*`, `orphans`, `widows`, and `print-color-adjust`. They do **not** give us Paged Media margin boxes (so running headers come from the browser's own print settings), `float: footnote` (so per-page footnotes are impossible), named pages, or column balancing. Anything needing the second list is out of scope by definition.

```css
@media print {
  .toolbar, .statusbar, .diagnostics, .editor-pane, .splitter,
  .find-bar, .cm-editor { display: none !important; }

  .preview-pane { position: static; width: 100%; overflow: visible; }
  :root { color-scheme: light; }
  body  { color: #000; background: #fff; }

  .book-title, .section-heading { break-after: avoid; }
  .chapter         { break-before: page; content-visibility: visible; }
  .poetry-line, tr { break-inside: avoid; }
  .verse           { orphans: 2; widows: 2; }

  .usfm-pb         { break-before: page; }   /* USFM's own \pb marker */
  a[href]::after   { content: ""; }
  .fig-placeholder { display: none; }
}
```

**The `content-visibility: visible` override is mandatory.** The preview uses `content-visibility: auto` for offscreen chapters ([ARCHITECTURE §10](ARCHITECTURE.md#10-preview-rendering)); the print renderer sees exactly that, so without the override printing yields one chapter and blank pages. It ships in the same commit as the optimization it corrects, with a test asserting page count on a twenty-chapter document.

Honouring `\pb` is not scope creep — USFM defines an explicit page-break marker, and print is the one place the preview legitimately knows about pages.

**Notes.** Per-page footnotes are unachievable without `float: footnote`. Rather than approximate badly, a setting offers *end of chapter* (default) or *end of document*, with the limitation stated in the print panel.

**Settings**, per document: page size (A4, or Letter under US/CA locale), margins (20 mm outer / 18 mm inner), base font size (11 pt), notes placement, include section headings, include introduction material, include cross-references (off), chapter starts new page. These generate an `@page` rule at print time.

**Mechanism.** `window.print()` on both targets — Tauri 2 has no native print API, the webview path is correct everywhere, and it yields Save as PDF for free. WebKitGTK's Linux print dialog is less capable around headers; documented, not worked around.

**Scope guard.** Columns, justification with hyphenation, column balancing, running headers with verse ranges — that is typesetting, and the answer is PTXprint.

---

## 9. Diagnostics

**Error**, **Warning**, or **Information**. Diagnostics never prevent saving — users must be able to save incomplete work.

Severity derives from the marker table ([ARCHITECTURE §6](ARCHITECTURE.md#6-the-marker-table)), not from hardcoded rules:

| Condition | Severity |
|---|---|
| Structurally invalid at any version | Error |
| Marker absent from the table and not `\z…` | Warning |
| Marker `deprecated_in` ≤ target version | Warning |
| Marker `since` > detected document version | Information |
| Unknown `\z…` marker | Information |
| 2.x positional `\fig` syntax | Warning + quick fix |

Conditions covered: missing or invalid book identification, unclosed character markers, unexpected closing markers, invalid nesting, duplicate chapter or verse markers, sequence problems, invalid attributes, unknown markers, deprecated markers, mixed line endings, mixed normalization, unsupported version constructs.

Every diagnostic carries a stable code, so it can be suppressed individually and its wording can change without breaking tooling. Codes defined elsewhere in these docs: `USFM-E018` (non-ASCII digits in `\v`), `USFM-I021` (mixed normalization), `USFM-W022` (joiner in a marker name), `USFM-I023` (joiner at a marker boundary).

---

## 10. Accessibility

Designed in from Phase 1 — retrofitting accessibility into a split-pane editor produces something that technically passes and is unusable.

Editor and preview are landmark regions with accessible names; F6 cycles; focus is ringed at 3:1 contrast. Diagnostic counts announce via `aria-live="polite"`, debounced to 1 s. Each preview verse carries `role="group"` with `aria-label="Genesis chapter 1 verse 1"`, so screen reader users navigate scripture structurally rather than as a text blob. Contrast is 4.5:1 for body text and 3:1 for UI and syntax; diagnostics use an underline plus a gutter glyph, never hue alone. `prefers-reduced-motion` disables scroll animation. Every panel releases focus on Escape. Usable at 200 % zoom with no horizontal scrolling of chrome.

---

## 11. Platform

**Desktop.** Native title bar, menus, and dialogs; system UI fonts for chrome ([UNICODE §7](UNICODE.md#7-fonts) for content); system themes; platform shortcuts. File associations for `.usfm` and `.sfm`.

**Web.** A Progressive Web App, because the offline requirement and a hosted deployment otherwise contradict. Precached shell, WASM binary, and fonts; no runtime caching needed since there are no runtime requests. Manifest `file_handlers` for `.usfm`/`.sfm` where supported. File System Access API where available, file input and blob download otherwise. New service workers install in the background behind a "A new version is ready — Reload" bar; never auto-reload mid-edit. The offline claim is scoped honestly: **after first load**, no network required.

**Updates.** Signed updater against a static endpoint, **opt-in on first run**, with the prompt stating it is the application's only network request. Never installs without consent or during an unsaved edit. An offline build variant with the updater compiled out exists for restricted deployments. Telemetry policy in [SECURITY §6](SECURITY.md#6-logging-and-telemetry).

## 12. Settings

TOML in the platform config directory, mirrored to `localStorage` on web. Human-editable.

Editor font family and size; per-script size multiplier; line height; theme; Backspace behaviour; show invisible characters; preview debounce; print notes placement, page size, margins; new-document template; suppressed diagnostic codes; update check enabled; recent files.

---

## 13. Acceptance criteria

What a user can observe. Part 4 of the [ROADMAP](ROADMAP.md) records the mechanism behind each one and where in the build it first holds.

1. A file can be opened, edited, previewed, and saved.
2. The editor shows the exact original source.
3. The preview updates within 120 ms p95 on a 2 MB file.
4. Malformed USFM produces diagnostics without crashing.
5. Unknown and custom markers survive a save unchanged.
6. An unchanged file, saved, is byte-for-byte identical.
7. Editing one verse and saving changes only that verse's lines.
8. Line endings, BOM, and normalization form are preserved, including per-line endings in mixed files.
9. A failed save never damages the previous file, and says where the previous content still is.
10. Unsaved work is recoverable after abrupt termination, with the prompt describing what was found.
11. Find, replace, undo, redo, and the §6.4 shortcuts work, with Find Next on ⌘G on macOS.
12. Cursor movement, selection, and deletion behave correctly across conjuncts and combining marks.
13. Find and Replace locates text regardless of the file's normalization form.
14. Typing through an input method produces no preview flicker and no dropped characters.
15. Documents in every corpus script open, render with correct shaping, and round-trip byte-for-byte.
16. Ctrl+P produces a complete, correctly paginated document.
17. Diagnostic severity reflects the document's USFM version, and every diagnostic can be suppressed by code.
18. The desktop application runs on Windows, macOS, and Linux.
19. The web application functions offline after first load.
20. No internet connection is required for normal editing.
21. The application is fully operable by keyboard alone.
22. No document content leaves the machine under any default configuration.
