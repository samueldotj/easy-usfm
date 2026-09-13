# Interface

What the window looks like, and why. PRODUCT says what the application does;
this says how it is arranged.

The arrangement comes from a Claude Design handoff, kept in `design/` — read
`design/usfm-editor-design-consultation/project/Easy USFM Redesign.dc.html` for
the mock-ups it was built from. The bundle is a prototype in HTML and CSS, not
production code; where this document and the mock-up disagree, this document is
what shipped and says why.

## 1. Two shells

The window has two arrangements. They are not two skins of one thing — they
answer different questions, and a translator moves between them during a
session. `View ▸ Layout` switches, and the choice is remembered.

**Workbench** is for building a file.

```
menu bar
document bar        John · Berean Standard Bible · 43_JHNBSB.usfm  ·  Save
┌──────────┬──────────────┬──────────────┬───────────┐
│ chapters │   editor     │   preview    │ inspector │
│ + outline│              │              │           │
└──────────┴──────────────┴──────────────┴───────────┘
diagnostics (docked)
status bar
```

**Study** is for reading what has been built. The navigator shrinks to a rail of
chapter numbers, the reading column takes the space that buys, and the
apparatus stops being a column: the note inspector floats beside the line it
describes, and the markers move to a strip along the bottom.

```
header      breadcrumb: John / Chapter 1 / The Witness of John / v 7
┌────┬──────────────┬─────────────────────┐
│ Ch │    editor    │       preview       │
│    │  ┌────────┐  │                     │
│    │  │ note   │  │                     │
│    │  └────────┘  │                     │
└────┴──────────────┴─────────────────────┘
marker strip
status bar
```

Both are the same editor, the same preview and the same engine. What differs is
how much room each is given and whether the apparatus is docked or floating.

Everything about the arrangement is remembered (`lib/layout.svelte.ts`): the
shell, which panes are showing, sync scroll, the navigator, the diagnostics
table. A layout is a working habit, not a property of the document — so none of
it is keyed on the file.

## 2. The palette is the fourth route

A command can be reached four ways: the menu, the toolbar, its shortcut, and
the palette (Ctrl+K). All four dispatch the same identifier, so none of them can
drift into being a second implementation of Save. `lib/commands.ts` is the
registry; the ids are the native menu's ids.

The palette's field answers four questions, told apart by the first character:

| Prefix | Means |
| --- | --- |
| *(none)* | a command |
| `:` | go to a reference |
| `\` | insert a marker |
| `@` | jump to a diagnostic |

Prefixes rather than a mode switch, because the typing starts before the
decision does. Each of the four still has its own dialog for someone who knew
which they wanted.

## 3. The panels are views of the parse

Nothing in the navigator or the inspector is stored, and nothing is pushed into
them. The chapter grid, the outline, the notes and the sidebars are all derived
from what the engine has already reported, so a chapter typed into existence
appears in the grid without anything being told to refresh.

All of it is scoped to the chapter the caret is in, which is what keeps the cost
flat: a book is two megabytes, a chapter is a few thousand characters, and every
scan runs over the chapter.

| Panel | Module | Derived from |
| --- | --- | --- |
| Chapter grid | `lib/outline.ts` | chunks + diagnostics |
| Outline | `lib/outline.ts` | the chapter's nodes |
| Notes | `lib/notes.ts` | the chapter's nodes |
| Sidebars | `lib/sidebars.ts` | the chapter's source |
| Marker at caret | `lib/markerAt.ts` | the caret's line |
| Book and translation | `lib/identity.ts` | the file's header |

A study sidebar's own `\ms` and any verse it quotes are apparatus, not the
chapter's: the outline and the verse count both skip them.

## 4. The sidebar editor writes back positionally

`\esb … \esbe` is the one construct in USFM that is really a small record — a
category, a heading and a body, written the same way hundreds of times in a
study Bible — so the inspector gives it fields.

Two rules make that safe.

**Nothing is reordered and nothing is dropped.** A real sidebar contains more
than three markers: poetry, a list, a figure, a second heading. The obvious
implementation reads the fields it knows, serialises them in a canonical order,
and quietly deletes the rest. So the rewrite is positional — each original line
is replaced in place by its edited form, and lines the panel does not model pass
through untouched and in order (ADR-003).

**Apply, not autosave.** The button replaces a span of the document through the
editor's own transaction, so it is one undo step and the buffer stays the
authority. A form that wrote as you typed would produce one undo entry per
keystroke in a text box.

The form follows the document until somebody edits the form. Editing the heading
in the source shows up in the panel; the moment a character is typed into a
field, the field is the user's and the document stops overwriting it until Apply
or a different sidebar.

## 5. Colour and type

`src/app.css` holds one palette in the design's own vocabulary (`--bg`, `--fg2`,
`--acc`) with the older semantic names (`--surface`, `--text-muted`) kept as
aliases. Three of the mock-up's tones were moved and only three, all for
contrast (PRODUCT §10) and all noted where they are defined:

| Token | Mock-up | Shipped | Why |
| --- | --- | --- | --- |
| `--fg3` (light) | `#a09a8e` | `#726a5a` | 2.6:1 on paper, and it sets the 10px labels |
| `--fg3` (dark) | `#736d63` | `#948d80` | 3.4:1 on charcoal, same reason |
| `--acc-text` | `#b3781c` | `#8a5b0c` | brass is 3.5:1 — a rule or a dot, not a 12px word |

`--acc` keeps the mock-up's brass for chrome, rules, dots and large type;
anything text-sized takes `--acc-text`. That is the same rule the Modernist
system states for its own accent.

Type is IBM Plex Sans for the interface, IBM Plex Mono for source, Literata for
Scripture — named rather than bundled, because `font-src 'self'` (SECURITY §4)
rules out a font CDN and the offline build (PRODUCT §12) must not make the
request at all. A system without them falls back to something with the same
proportions.

The editor and the preview take different stacks, and both begin with the
script-aware content family. Latin and Cyrillic are deliberately absent from the
script table, so they fall through to Plex Mono in one and Literata in the
other, while every complex script is claimed by a `unicode-range` before either
tail can be reached. UNICODE §7 is untouched: no complex script can land in a
monospace face.

## 6. Keyboard

Everything below is in the menu, with the accelerator declared there and only
there on the desktop — so the shortcut shown beside a menu item is the one that
runs (PRODUCT §6.4). Command on macOS wherever the table says Ctrl.

| Key | Command |
| --- | --- |
| Ctrl+K | Command palette |
| Ctrl+G (⌘L) | Go to reference |
| Ctrl+F / Ctrl+H | Find / Replace |
| F3 / Shift+F3 | Find next / previous |
| F8 / Shift+F8 | Next / previous diagnostic |
| F6 | Cycle pane focus |
| Ctrl+1 / Ctrl+2 | Focus editor / preview |
| Ctrl+Shift+M | Diagnostics |
| Ctrl+Shift+8 | Show invisible characters |
| Ctrl+Shift+F | Insert footnote |
| Ctrl+B / Ctrl+I | Bold / Italic |
| Ctrl+± / Ctrl+0 | Zoom |
| F1 | Marker reference |

Lists that could grow long are one tab stop with arrow keys within — the chapter
grid, the outline, the diagnostics table, the palette. A hundred and fifty
chapters as a hundred and fifty tab stops technically passes an audit and is
unusable in practice (PRODUCT §10).
