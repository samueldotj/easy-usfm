# Roadmap

The shape of the work and why it is ordered as it is.

**This is not a status board.** Nothing here records what is finished — that lives in the issue tracker and the git history. What is written here should still be true and still be useful after version 1.0 ships: what each stage delivers, why the sequence is what it is, and where each guarantee first gets established.

Related: [PRODUCT](PRODUCT.md) · [ARCHITECTURE](ARCHITECTURE.md) · [FILE-FIDELITY](FILE-FIDELITY.md) · [UNICODE](UNICODE.md) · [SECURITY](SECURITY.md)

---

# Part 1 — Milestones

What someone can do at each stage, and what they still cannot.

| | Milestone | For | What it means |
|---|---|---|---|
| **M0** | Foundations | nobody | The engine reads USFM correctly. No interface. |
| **M1** | An editor you can trust | us | Open, edit, and save any USFM file without risk of losing it. |
| **M2** | It understands USFM | us | Highlighting, error reporting, verse navigation, marker help. |
| **M3** | You can see your Scripture | first outside testers | Live preview and printing. The product becomes itself. |
| **M4** | Reliable on a real machine | wider testers | Survives crashes, cloud folders, and files changed underneath you. |
| **M5** | Runs in a browser | anyone with a link | Installable web app, works offline. |
| **M6** | Version 1.0 | everyone | Signed installers, accessible, verified across scripts. |

### M0 — Foundations

*No user-facing output.* The parser reads real USFM from hundreds of real files, reports precise errors, tracks where every construct sits in the source, and reparses one chapter rather than the whole book when you type.

It comes first because it is the largest and least parallelizable piece, it can be validated with no interface at all, and everything after it assumes it works.

### M1 — An editor you can trust

**You can** open a `.usfm` or `.sfm` file, edit it as plain text, and save it. Create new files. Use familiar shortcuts. Read and write in any script without the text mangling.

**You cannot yet** see a preview, get error reporting, or navigate by verse. It does not know USFM is USFM — it is a plain text editor.

It counts as a milestone because the file is safe. An unchanged file saves byte-for-byte identically; a failed save never damages what was there; line endings, byte-order marks, and Unicode form survive untouched. That is the property users cannot check for themselves and cannot forgive, which is why it is built before anything visible.

### M2 — It understands USFM

**You can** see markers highlighted, get errors and warnings at the exact place they occur, jump to any chapter and verse, autocomplete markers as you type `\`, and find and replace text reliably regardless of how the file encodes its characters.

**You cannot yet** see formatted Scripture. The right-hand pane is not there.

Still an internal build: a good USFM text editor, not yet the product.

### M3 — You can see your Scripture

**You can** read a formatted preview beside the source — chapters, verses, poetry, footnotes, cross-references, tables, headings. Click a verse in the preview to jump to it in the source. Print a clean copy or save it as PDF.

**You cannot yet** rely on it. Crash recovery, external-change detection, and cloud-folder handling are not in.

**This is the first milestone worth putting in front of someone outside the project.** It does what the name promises. Suitable for a small number of friendly testers who are not editing anything irreplaceable.

### M4 — Reliable on a real machine

**You can** close the laptop, have the application crash, and get your work back. Keep files in Dropbox, OneDrive, or a network share without conflicted copies. Get told when something else changes the file underneath you, with a real choice about what to do.

**You cannot yet** use it in a browser.

This is where it becomes safe to recommend to people editing their own working files.

### M5 — Runs in a browser

**You can** open a link, use the editor without installing anything, and keep working with the network off.

It is a small milestone because the engine and the whole interface already run in the browser from M2 onward — there is one engine on all targets ([ADR-002](adr/002-wasm-or-native.md)). This is packaging, not a port.

### M6 — Version 1.0

**You can** install a signed build on Windows, macOS, or Linux. Operate the whole application by keyboard. Trust that nothing you type leaves the machine.

Every criterion in [PRODUCT §13](PRODUCT.md#13-acceptance-criteria) demonstrated.

---

# Part 2 — How the work is shaped

## Why this order

Three sequencing decisions drive everything below, and each is a departure from the obvious order.

**The parser goes first and alone.** It is the largest and riskiest piece, it barely parallelizes, and it needs no interface to validate. Building the shell first would mean discovering parser constraints through a UI, which is the expensive way to find them.

**File safety comes before features.** M1 delivers a text editor that does not understand USFM at all, which looks like a wasted phase and is not. Byte-exact round-tripping, the atomic save ladder, and fidelity preservation are properties users cannot verify for themselves and cannot forgive. They are also nearly impossible to retrofit under a product that already has users.

**Accessibility and complex-script support land in Phase 1, not at the end.** Both cost roughly four times as much retrofitted. Logical CSS properties, grapheme-aware interaction, and a font strategy are cheap when designed in and expensive to bolt on — the second requires auditing every stylesheet and every cursor operation written in the interim.

## Sizing

Sizes are relative effort, not schedule. They exist to signal where the risk sits, not to support a Gantt chart.

| | Meaning |
|---|---|
| **S** | A focused session. Well understood before it starts. |
| **M** | A few sessions. The common case. |
| **L** | Wide surface or genuinely intricate. Expect a second pass. |
| **XL** | Where the project can go wrong. A poor approach here costs weeks, not days. |
| **⏳** | Lead-time-bound. Little effort, unpredictable calendar. Orthogonal to size. |

61 items: 15 S, 37 M, 7 L, 2 XL. For a rough sense of scale, that is a few months of full-time work for one person. Phase 0 is largely serial; later phases have parallel tracks.

**The two XL items are worth knowing by name.** P0.4, the chapter-chunked incremental session, is where the performance budget is won or lost. P2.2, the delta protocol and mirrored buffer, is where silent state corruption enters if it enters at all. Both deserve a design pass before code.

## Items with lead time

Three items are gated by something outside the work itself. Their calendar is not their effort, so they benefit from being started well before their phase.

| ID | Gated by | Typical wait |
|---|---|---|
| **P0.1** | Licence terms for some translations are undiscoverable; clarifying means contacting rights holders | days to weeks |
| **P6.2** | EV code-signing certificates must be purchased and issued | 3–10 business days |
| **P6.5** | Requires physical Windows and macOS machines for shaping verification | procurement |

## Reading the item tables

Each item is one coherent deliverable. **Done includes tests and green CI**, not code that runs locally. The *Done when* column is the acceptance test — it is written to be checkable by someone who did not do the work.

The design assumes the [`usfm3`](https://crates.io/crates/usfm3) crate as the parsing foundation ([ADR-001](adr/001-parser.md)), pinned to an exact version and wrapped behind our own facade.

---

# Part 3 — Work items

## Phase 0 → M0 · Parser

*11 items · 4 S, 5 M, 1 L, 1 XL*

| ID | | Deliverable | Done when |
|---|---|---|---|
| **P0.1** | S ⏳ | Corpus: ~200 vendored files, licence audit, fetch script for the extended set, verify harness | ~200 files vendored covering ≥ 12 scripts and every feature class; redistribution terms confirmed for each; `just corpus-fetch` retrieves the extended set; `just corpus-verify` re-hashes and passes |
| **P0.2** | S | `easy-usfm-core` skeleton; our `Node`, `Diagnostic`, `Span` types; `usfm3` pinned and wrapped | Facade compiles for host and wasm32; no `usfm3` type appears in the public API |
| **P0.3** | M | `Utf16Mapper` and the `Char16` boundary type | Byte offsets cannot serialize (compile error if attempted); all four [UNICODE §9.1](UNICODE.md#91-offset-property-tests) properties green |
| **P0.4** | **XL** | Chapter chunking and the incremental session | Single-chapter edit reparses one chunk; `\c` insert and delete split and merge correctly; reparse < 15 ms on a 2 MB file |
| **P0.5** | S | Chunk-boundary equivalence test | Chapter parsed in isolation ≡ parsed in situ across the corpus, plus the five targeted edit cases in [ARCHITECTURE §12.2](ARCHITECTURE.md#122-chunk-boundary-equivalence) |
| **P0.6** | L | `markers.toml` generated from the specification, plus loader | ~200 rows with `class`, `closing`, `since`, `deprecated_in`, `nests_under`, attributes; generator re-runnable against a spec revision |
| **P0.7** | M | Diagnostic codes and version-derived severity | Every condition in [PRODUCT §9](PRODUCT.md#9-diagnostics) emits a stable code with correct severity; suppression by code works |
| **P0.8** | M | Verse index and range model | `\v 1-2`, `\v 1a`, `\va`, `\vp` parsed; range-overlap duplicates detected; gaps reported as Information |
| **P0.9** | M | Normalization search index | NFC index with an offset map back to raw; rebuilds on the dirty-chunk schedule; an NFC query finds NFD text |
| **P0.10** | M | Three-way differential harness | Corpus diffed across our layer, `usfm3` direct, and `usfm-grammar`; zero unexplained structural diffs |
| **P0.11** | S | CLI, fuzz target, benchmark harness | `parse` / `diagnostics` / `usj` / `bench` over a directory; `cargo-fuzz` target runs clean for 24 h; 2 MB parses < 400 ms native and < 700 ms wasm |

Ongoing, not an item: stay in contact with the `usfm3` maintainer over incremental reparse, feature flags, and advance notice of breaking changes.

## Phase 1 → M1 · Shell and file safety

*12 items · 2 S, 8 M, 2 L*

| ID | | Deliverable | Done when |
|---|---|---|---|
| **P1.1** | M | Tauri 2 + Svelte + Vite + TypeScript scaffold; CI matrix | Windows, macOS, and Linux builds green in CI from an empty window |
| **P1.2** | M | CodeMirror 6 integration, split pane, resizable divider, themes | Editor accepts input; divider drags and persists; light, dark, and system themes |
| **P1.3** | S | `FileFidelity` capture and serialization | BOM, uniform EOL, and final-newline state round-trip on a fixture set |
| **P1.4** | L | Mixed-EOL per-line tracking through change mapping | Per-line terminators survive edits that split and join lines; new lines inherit correctly |
| **P1.5** | M | Atomic save rung 1, POSIX | Temp in target directory, `fsync` of file and parent, xattrs and ACLs copied, symlinks resolved |
| **P1.6** | M | Atomic save rung 1 Windows (`ReplaceFileW`), plus rung 3 | ADS and ACLs preserved across save; read-only target offers Save As instead of failing |
| **P1.7** | L | Rung 2 copy-back; sync-root and hardlink detection | Inode preserved; sidecar written and cleaned; `st_nlink > 1` forces rung 2; status bar shows the reason |
| **P1.8** | M | `FileSystem` trait and `FaultyFs` injection suite | All six fault cases in [FILE-FIDELITY §5.2](FILE-FIDELITY.md#52-fault-injection) leave the original intact and the document dirty |
| **P1.9** | S | T1–T3 round-trip tests in CI over the corpus | All three pass on every corpus file; the T3 diff touches only the edited verse |
| **P1.10** | M | New, Open, Save, Save As; dirty state; native dialogs and menus | Full file lifecycle; unsaved-change warning on close; recent files |
| **P1.11** | M | Font stack, per-script sizing, gutter/content split, missing-font notice | Complex scripts render without tofu on a clean VM; gutter stays monospace |
| **P1.12** | M | Logical-property lint; static CodeMirror theme against the real CSP; accessibility baseline | Lint fails on physical properties; no runtime `<style>` injection; landmarks, F6 cycling, focus ring, contrast audit pass |

## Phase 2 → M2 · Engine integration

*12 items · 4 S, 7 M, 1 XL*

| ID | | Deliverable | Done when |
|---|---|---|---|
| **P2.1** | M | `easy-usfm-wasm` bindgen surface, worker bootstrap, build pipeline | Worker loads the module and answers a parse request on both targets |
| **P2.2** | **XL** | Delta protocol — `ChangeSet` → `EditBatch`, mirrored buffer | Mirror matches the editor after a scripted 10,000-edit session |
| **P2.3** | S | Composition suppression | Synthetic IME sequence emits exactly one batch; checksum matches; no preview flicker. **Merges with P2.2, not after it** — shipping the protocol without this means input-method users desync on every word |
| **P2.4** | S | Checksum desync detection and resync paths | Induced drift caught within 50 batches and repaired without data loss |
| **P2.5** | S | Revision discipline and request cancellation | Stale results never applied under a scripted fast-typing load |
| **P2.6** | M | Syntax highlighting from the real token stream | Markers, attributes, and text distinguished; marker tokens bidi-isolated |
| **P2.7** | M | Diagnostics panel, gutter decorations, F8 navigation | Diagnostics land on the right characters in mixed-script text |
| **P2.8** | S | Version detection and tiered strictness wiring | Status bar shows detected version; severity shifts correctly when overridden |
| **P2.9** | M | Go to Reference, chapter and verse navigation, status bar | All accepted reference forms resolve, including `\vp` fallback and non-ASCII digits |
| **P2.10** | M | Marker autocomplete with context ranking | Valid-in-context first; deprecated greyed with replacement, never ranked first |
| **P2.11** | M | Find and Replace over the normalized index | NFC query finds NFD text; exact-byte toggle works; replace inserts text unmodified |
| **P2.12** | M | Web document service — FSA API, file input, blob download | Same lifecycle as desktop in a browser, with the documented limitations |

## Phase 3 → M3 · Preview

*12 items · 4 S, 6 M, 2 L*

| ID | | Deliverable | Done when |
|---|---|---|---|
| **P3.1** | M | Preview model and base node components — chapter, verse, paragraph, character styles | A simple book renders correctly; no `{@html}` anywhere |
| **P3.2** | M | Notes — `\f`, `\fe`, `\ef`, `\efe`, `\x`, `\ex` | Callers, note content, and origin references render; nested `\+` markers correct |
| **P3.3** | M | Poetry, lists, tables | Indentation levels, list headers and footers, aligned table cells |
| **P3.4** | L | Sidebars, figures, introductions, milestones, raw placeholders | Unpaired milestones warn and render as a chip; unclassifiable content shows verbatim. **Depends on the deferred decisions in Part 5** |
| **P3.5** | M | Chunked keyed rendering, `content-visibility`, first-paint overscan | 2 MB document scrolls at 60 fps; first paint < 800 ms p95 |
| **P3.6** | L | Click-to-source and scroll sync, both directions | Click lands on the right character across conjuncts and reordered vowel signs |
| **P3.7** | S | URL sanitizer, inert rendering, external-link confirmation | `javascript:`, `data:`, and protocol-relative payloads render as inert text |
| **P3.8** | M | Figure policy — off by default, local only, scoped asset protocol | `..` traversal rejected including encoded forms; remote `src` never fetched |
| **P3.9** | S | CSP enforced on both targets; `{@html}` lint and CI grep | Both builds run under the real policy with no console violations |
| **P3.10** | M | Print stylesheet and the `content-visibility` override | Twenty-chapter document prints every chapter; page count asserted in CI |
| **P3.11** | S | Print settings panel and `@page` generation | Page size, margins, and notes placement apply; settings persist per document |
| **P3.12** | S | Invisible characters; non-ASCII digit reference parsing | Joiners visible and diagnosed; `௩:௧` and `3:1` resolve identically |

## Phase 4 → M4 · Robustness

*6 items · 5 M, 1 L*

| ID | | Deliverable | Done when |
|---|---|---|---|
| **P4.1** | M | Recovery snapshots — cadence, retention, pruning | Snapshot every 4 s idle and 45 s under continuous typing; last 3 kept; cleared on clean save |
| **P4.2** | L | Advisory lock, multi-window, crash detection, recovery prompt | SIGKILL then reopen offers recovery with an accurate diff summary; second window on the same file behaves per [FILE-FIDELITY §4](FILE-FIDELITY.md#4-recovery-and-locking) |
| **P4.3** | M | File watcher with hash-based self-suppression | Own saves never prompt; genuine external edits always do |
| **P4.4** | M | External-change UX | Clean reload preserves position by verse reference; dirty shows the non-modal bar; deleted file handled |
| **P4.5** | M | Pathological corpus, fuzzing in CI, pinned benchmarks | Every pathological case handled; nightly fuzz over the extended corpus; > 20 % perf regression fails the build |
| **P4.6** | M | Web recovery parity — IndexedDB and `navigator.locks` | Snapshot survives a tab crash; cross-tab lock prevents double editing |

## Phase 5 → M5 · Web completion

*2 items · 1 S, 1 M*

| ID | | Deliverable | Done when |
|---|---|---|---|
| **P5.1** | M | Service worker, precache, update bar | Full editing session with the network disabled after first load; update bar never auto-reloads mid-edit |
| **P5.2** | S | Manifest, `file_handlers`, install flow, static hosting build | Installs as a PWA; opens `.usfm` from the OS where supported |

## Phase 6 → M6 · Release quality

*6 items · 5 M, 1 L*

| ID | | Deliverable | Done when |
|---|---|---|---|
| **P6.1** | M | Native menus per platform | Menu structure matches platform convention; all shortcuts reachable |
| **P6.2** | M ⏳ | Installers and code signing, three platforms | Signed artefacts install cleanly on a fresh machine each |
| **P6.3** | M | Opt-in updater and offline build variant | Update prompt states it is the only network request; offline variant makes zero requests |
| **P6.4** | L | WCAG 2.2 AA audit and fixes | Audit passes; full keyboard-only operation |
| **P6.5** | M ⏳ | Security checklist; shaping verification on real hardware | [SECURITY §7](SECURITY.md#7-release-checklist) complete; shaping verified on real Windows and macOS installs |
| **P6.6** | M | Documentation, release notes, v1.0 sign-off | Every criterion in [PRODUCT §13](PRODUCT.md#13-acceptance-criteria) demonstrated |

---

# Part 4 — Where each guarantee is established

User-visible criteria are in [PRODUCT §13](PRODUCT.md#13-acceptance-criteria). This table records the mechanism behind each one and the point in the build where it first holds — useful long after the work is done, because it says which test protects which promise.

| Guarantee | Mechanism | Established at |
|---|---|---|
| Offsets correct across scripts | `proptest`, four properties | P0.3 |
| Chunked parsing agrees with whole-document parsing | corpus-wide CST equivalence | P0.5 |
| No interpretation drift from the ecosystem | zero unexplained diffs, three-way oracle | P0.10 |
| Parser never panics | 24 h clean `cargo-fuzz` | P0.11, held by P4.5 |
| Failed saves never damage the original | fault injection, all three rungs | P1.8 |
| Byte-exact round trip | T1–T3 across the corpus | P1.9 |
| Same WASM artifact on desktop and web | build hash comparison | P2.1 |
| Composition produces one batch | synthetic IME test | P2.3 |
| Preview update, 2 MB, single-chapter edit | < 120 ms p95 | P3.5 |
| First preview paint, 2 MB cold open | < 800 ms p95 | P3.5 |
| Hostile `link-href` renders inert | corpus fixture | P3.7 |
| No raw-markup path from document content | CI grep and lint rule | P3.9 |
| Recovery after abrupt termination | SIGKILL test with prompt assertion | P4.2 |
| Watcher does not fire on own writes | self-suppression test, both directions | P4.3 |
| Performance does not regress | CI benchmark, > 20 % fails the build | P4.5 |
| No network requests with updater disabled | capture during a full session | P6.3 |
| Keyboard-only operation | WCAG 2.2 AA audit | P6.4 |
| Shaping across corpus scripts | manual, real Windows and macOS | P6.5 |

---

# Part 5 — Decisions deferred

Left open at design time because answering them well needs the preview built far enough to see the consequences. Both shape P3.4.

1. **`\periph` and peripheral divisions.** In the specification, absent from the preview scope. Render them, or pass them through as raw content?
2. **Extended notes and sidebars** (`\ef`, `\efe`, `\esb`, `\cat`). Structurally unlike ordinary footnotes — they carry block content rather than inline. Needs a preview design pass before the item can be sized honestly.

# Part 6 — Deliberately excluded

Recorded so the boundary is defensible when the request arrives, and so a future reader can see these were considered rather than overlooked.

| Request | Answer |
|---|---|
| Columns, justification, hyphenation, running headers with verse ranges | Typesetting. PTXprint does this; we do not. |
| Multi-book projects, translation assignments | Out of scope; this is a file editor. |
| Paratext or Git integration | Out of scope. |
| RTL *interface* mirroring | Deferred, with the logical-property hedge kept so it stays cheap to add ([UNICODE §8](UNICODE.md#8-text-direction)). Document direction is supported. |
| Mobile applications | Out of scope. |
| Default-on crash reporting | Never. [SECURITY §6](SECURITY.md#6-logging-and-telemetry). |

---

## Keeping this document true

The item tables will drift as the work reveals itself — items split, merge, and get reordered, and that is expected. The parts meant to outlast them are **Part 1** (what each milestone delivers), **Part 2's ordering rationale**, and **Part 4** (which test protects which promise). If an item changes, update the table. If one of those three stops being true, something more significant has changed and it deserves a note in the relevant [ADR](adr/).
