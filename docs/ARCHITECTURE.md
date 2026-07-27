# Architecture

Layering, the engine, the parser facade, incremental parsing, the delta protocol, preview rendering, performance, and engine testing.

Related: [PRODUCT](PRODUCT.md) · [FILE-FIDELITY](FILE-FIDELITY.md) · [UNICODE](UNICODE.md) · [SECURITY](SECURITY.md) · [ROADMAP](ROADMAP.md)

Decisions and their rejected alternatives live in the ADRs: [001 parser](adr/001-parser.md) · [002 WASM or native](adr/002-wasm-or-native.md) · [003 source authoritative](adr/003-source-authoritative.md) · [004 USJ model](adr/004-usj-model.md)

---

## 1. Stack

```text
Desktop shell     Tauri 2 + Rust  (file I/O, dialogs, watching, recovery)
Frontend          Svelte + Vite + TypeScript  (no SvelteKit)
Editor            CodeMirror 6
USFM engine       Rust → WebAssembly, in a Web Worker, on all targets
USFM parser       usfm3 crate, behind our own facade
Document model    USJ
```

SvelteKit is omitted deliberately: a single-window editor needs no router, no SSR, and no file-based routing, and Vite's static output is consumed unchanged by both the Tauri bundle and the static host.

## 2. Layering

```text
                    ┌──────────────────────────────┐
                    │  easy-usfm-core (Rust)       │
                    │  facade: session, incremental │
                    │  offsets, diagnostics, index │
                    │        ↓ wraps ↓             │
                    │  usfm3 (pinned dependency)   │
                    └──────────────┬───────────────┘
                                   │ wasm32-unknown-unknown
                                   ▼
                    ┌──────────────────────────────┐
                    │  easy-usfm-wasm (Web Worker) │   identical on all targets
                    └──────────────┬───────────────┘
                                   │ postMessage — edit deltas only
                    ┌──────────────▼───────────────┐
                    │  Svelte UI + CodeMirror 6    │
                    └──────────────┬───────────────┘
                                   │ platform service interfaces
                 ┌─────────────────┴─────────────────┐
                 ▼                                   ▼
   easy-usfm-tauri (file I/O only)        Browser (FSA API / download)
```

Svelte components never call Tauri APIs directly:

```typescript
export interface DocumentService {
  open(): Promise<OpenedDocument | null>;
  save(doc: EditableDocument): Promise<SaveOutcome>;
  saveAs(doc: EditableDocument): Promise<SaveOutcome>;
}
```

`documentService.tauri.ts` and `documentService.web.ts` are selected at build time. Nothing in the component tree branches on platform.

**One engine, compiled once.** The same WASM artifact runs on desktop and web; there is no native parsing path. Native Rust handles only what browsers cannot: dialogs, reading, atomic writing, watching, recovery. Reasoning in [ADR-002](adr/002-wasm-or-native.md).

## 3. Crates

```text
crates/
├── easy-usfm-core/   facade API, incremental session, Char16 offsets,
│                     diagnostic codes, verse index, normalization index
│                     internal dependency: usfm3 (pinned exactly)
├── easy-usfm-wasm/   wasm-bindgen surface, worker protocol
└── easy-usfm-tauri/  file access, atomic save, recovery, watching, commands
```

`easy-usfm-core` builds standalone with no Tauri and no filesystem access, and is a candidate for its own repository — CI in seconds rather than minutes, platform-independence enforced structurally, and the Phase 0 CLI becomes a debugging tool used throughout every later phase.

**The facade boundary is load-bearing.** [ADR-001](adr/001-parser.md)'s risk controls depend on nothing above `easy-usfm-core` knowing which parser sits underneath.

---

## 4. Source-of-truth model

```text
USFM source text  ← authoritative, byte-exact
       ↓
Tolerant parser
       ↓
USJ document tree
       ↓
Diagnostics · chapter/verse index · preview model · source mappings
       ↓
Svelte preview
```

The arrow points one way. **Easy USFM never serializes a document from its tree** — saving writes the buffer with the fidelity envelope reapplied ([FILE-FIDELITY §1](FILE-FIDELITY.md#1-the-fidelity-envelope)). Byte-exactness is therefore a property of the architecture, not of a dependency. See [ADR-003](adr/003-source-authoritative.md).

**Coordinate spaces.** Byte offsets never leave `easy-usfm-core`; `usfm3` emits them and they stop at the facade. Everything crossing to JavaScript is UTF-16 code units. Full contract in [UNICODE §1](UNICODE.md#1-three-coordinate-spaces).

## 5. Document model

The tree is the USJ content model ([ADR-004](adr/004-usj-model.md)), extended with source spans and a variant for unclassifiable content:

```rust
pub struct Node {
    pub kind: UsjKind,          // book | chapter | verse | para | char | note
                                // | ms | table | sidebar | figure | text
    pub marker: Option<Tag>,
    pub attributes: Vec<Attribute>,
    pub span: ByteRange,        // internal; Char16 at the boundary
    pub children: Vec<Node>,
    pub raw: Option<RawSpan>,   // preserved verbatim
}
```

## 6. The marker table

Marker semantics live in data. `markers.toml`, generated from the specification and checked in:

```toml
[bd]
class = "character"; closing = "explicit"; since = "1.0"
nests_under = ["*"]            # "*" = any character or note context

[ph]
class = "paragraph"; closing = "implicit"; since = "1.0"
deprecated_in = "3.0"; replacement = "pi#"

[jmp]
class = "character"; closing = "explicit"; since = "3.0"
attributes = ["link-href", "link-title", "link-id"]; default_attr = "link-href"
```

`closing` is one of `explicit | implicit | milestone | none`. Diagnostic severity derives from `since` and `deprecated_in` ([PRODUCT §9](PRODUCT.md#9-diagnostics)).

Whether `usfm3`'s own `markers` module carries version metadata is a Phase 0 question; ours supplies it regardless. Deprecated in the specification and present in the initial table: `\ph` (use `\pi#`), `\addpn`, `\pro`.

## 7. Verse index

```rust
pub struct VerseId { pub start: u16, pub end: u16, pub segment: Option<char> }

pub struct VerseEntry {
    pub chapter: u16,
    pub verse: VerseId,
    pub published: Option<String>,    // \vp — may use non-ASCII digits
    pub alternate: Option<VerseId>,   // \va
    pub span: Char16Range,
}
```

`usfm3`'s `vref` module is the starting point; its depth on ranges and alternates is a Phase 0 question. Behaviour in [PRODUCT §6.2](PRODUCT.md#62-verse-references).

---

## 8. Parsing

### 8.1 Facade

`easy-usfm-core` wraps [`usfm3`](https://crates.io/crates/usfm3), pinned exactly, exposing our own API. Its public API is staged, and the staging is real — `ParsedDocument` uses `OnceCell` per stage, so the cheap path does not pay for the expensive one:

| Our tier | `usfm3` |
|---|---|
| Per-keystroke lexing | `tokenize()` |
| Structural parse | `parse_cst()` — source-backed, lossless |
| Semantic passes and diagnostics | `parse_ast(…, diagnostics: true)` |
| Document model | `to_usj()` |

`usfm3` is an implementation detail of `easy-usfm-core`: pinned to an exact version, never named in the public API, and swappable without touching anything above the facade. [ADR-001](adr/001-parser.md).

### 8.2 Incremental parsing

Nothing is O(document) on a keystroke except lexing, which is O(edited lines). Two properties of USFM make this tractable: structural markers are line-initial, so lexing is line-local and cacheable; and **`\c` at line start is a hard synchronization point**, since no construct legally spans a chapter boundary.

**Tier 1 — token cache.** One `LineTokens` per line keyed by content hash, from `usfm3::tokenize`. A single-character edit relexes one line.

**Tier 2 — chapter-scoped parse.** The document partitions at `\c` into chunks, plus a header chunk for everything before `\c 1`. An edit marks its chunk dirty; `parse_cst` runs on that chunk's range only and the CST is spliced. Inserting or deleting a `\c` splits or merges neighbours — the only case where more than one chunk reparses.

```rust
pub struct ChapterChunk {
    pub number: Option<u32>,          // None for the header chunk
    pub byte_range: Range<usize>,
    pub rev: u64,
    pub cst: CstDocument,
    pub diagnostics: Vec<Diagnostic>,
}
```

**Tier 3 — cross-chunk index.** Verse sequencing, duplicate detection, and the chapter/verse index derive from chunk summaries rather than the full tree. O(chunks), cheap enough to run unconditionally.

Chunk-boundary correctness is this layer's one real hazard and gets its own test (§11.2).

### 8.3 Revision discipline

Every worker message carries a monotonic `rev: u64`. The main thread holds `latestAppliedRev` and discards lower-revision results; superseded requests are cancelled in the worker queue. Stale results never replace newer state.

```typescript
interface ParseResult {
  rev: number;
  dirtyChunks: ChunkPatch[];    // only what changed
  diagnostics: Diagnostic[];
  verseIndex: VerseIndexPatch;
}
```

## 9. Delta protocol

The text is mirrored, not shipped. CodeMirror is authoritative for editing; the worker holds a synchronized buffer. Sending a 2 MB string per debounce would mean a transcode and allocation on every edit.

```typescript
type Edit = { fromA: number; toA: number; insert: string };   // Char16

interface EditBatch {
  rev: number;
  edits: Edit[];
  checksum?: string;      // xxh3 of the full document, sent opportunistically
}
```

Produced directly from the transaction via `update.changes.iterChanges(…)`.

**Desync detection.** Silent drift corrupts every offset in the interface. Every 50 batches, and at each idle boundary, the checksum is included; on mismatch the worker replies `{ desync: true }` and the main thread ships a full resync. Full resync is also used on open, on undo crossing more than 1000 changes, and after external reload.

**Composition suppression.** Edits are buffered and not transmitted during input-method composition. A correctness requirement, not an optimization — without it the mirror receives uncommitted text and can desync permanently. [UNICODE §5](UNICODE.md#5-input-method-composition).

## 10. Preview rendering

```svelte
{#each chunks as chunk (chunk.number)}
  <ChapterView {chunk} />
{/each}
```

**Keyed each block** — only the chunk whose `rev` changed re-renders. **Offscreen containment** — `.chapter { content-visibility: auto; contain-intrinsic-size: auto 800px; }` lets the browser skip layout, style, and paint for offscreen chapters, which is what makes a 2 MB document scroll smoothly and why print needs an explicit override ([PRODUCT §8](PRODUCT.md#8-print)). **First paint** — chapters intersecting the viewport plus one screen of overscan render immediately; the rest parses in the background.

The preview is built from typed model nodes rendered by Svelte components. `{@html}` is banned and the ban is lint-enforced — [SECURITY §1](SECURITY.md#1-the-preview-never-executes-content).

## 11. Performance

| Metric | Target | On |
|---|---|---|
| Keystroke → editor paint | < 16 ms, always | any size |
| Typing idle → preview updated (single-chapter edit) | < 120 ms p95 | 2 MB |
| Cold open → first preview paint | < 800 ms p95 | 2 MB |
| Cold open → full parse and index | < 2.5 s p95 | 2 MB |
| Save (400 KB) | < 300 ms p95 | local disk |
| Peak resident memory | < 6× file size | 2 MB |

The engine runs in a dedicated Web Worker on both targets; the main thread never parses. "Typing is never blocked by parsing" is satisfied structurally rather than by hope. A CI benchmark over a fixed corpus file fails the build on a regression greater than 20 %.

---

## 12. Engine testing

Round-trip and save-failure tests are in [FILE-FIDELITY §5](FILE-FIDELITY.md#5-testing); offset and script tests in [UNICODE §9](UNICODE.md#9-testing).

### 12.1 Three-way differential oracle

Two independent mature implementations emit USJ — the model we adopt. A dev-only harness parses the corpus with all three and diffs structurally:

```text
corpus file ──┬──► easy-usfm-core   ──► normalized USJ
              ├──► usfm3 (direct)   ──► normalized USJ    ← in-process, no CLI
              └──► usfm-grammar     ──► normalized USJ
                                              │
                                              ▼
                                     three-way structural diff
```

Ours versus `usfm3` isolates chunking bugs — if chunked parsing disagrees with whole-document parsing, the fault is ours. Both versus `usfm-grammar` isolates genuine interpretation differences. Two of the three are already Rust dependencies, so this costs almost nothing.

### 12.2 Chunk-boundary equivalence

Parsing a chapter in isolation must produce the same CST as parsing it in the whole document, asserted across the corpus, plus targeted cases: inserting `\c`, deleting `\c`, splitting a chapter, editing the header chunk, editing at the exact boundary.

### 12.3 Fuzzing

"Malformed USFM produces diagnostics without crashing" is a fuzzing claim, and only fuzzing establishes it. `cargo-fuzz` over arbitrary bytes asserts: never panics; always returns a tree, however degenerate; every offset in bounds and on a UTF-8 boundary; parse time sub-quadratic. Run against both our layer and `usfm3` directly. A 24-hour clean run gates each release.

### 12.4 Corpus

Fixtures do not find parser bugs; real files do. The corpus comes in two tiers.

**Core — about 200 files, vendored by hash and committed.** Chosen deliberately rather than sampled: every script in the coverage list below, and every feature class (notes, poetry, tables, lists, milestones, attributes, sidebars, figures, peripherals, `\zaln` alignment data). Runs on every push. Around 20 MB, which a repository can carry without becoming unpleasant to clone.

**Extended — the long tail, fetched not committed.** `just corpus-fetch` retrieves it from eBible.org and unfoldingWord against a checksum manifest. Runs nightly, not per-push. This is where breadth lives: hundreds of translations that would add nothing to a pull-request cycle but do catch the construct nobody anticipated.

Both tiers are pinned by checksum, so `just corpus-verify` proves nothing has drifted. Vendoring the core tier rather than fetching it keeps per-push CI hermetic and offline; keeping the extended tier out of the repository avoids carrying hundreds of megabytes in every clone forever. It also isolates the licensing problem — the 200 committed files need confirmed redistribution terms, while the fetched tier only needs to be readable where it is published.

Script coverage must include Latin, Greek, Cyrillic, Hebrew, Arabic, Devanagari, Tamil, Bengali, Thai, Khmer, Myanmar, and Han or Hangul — chosen to exercise combining marks, conjunct formation, visual reordering, RTL, and scripts without word spacing.

**Pathological set:** BOM + CRLF + no trailing newline; mixed line endings; unclosed `\bd`; notes nested four deep; `\c` with no `\v`; a 40,000-line single chapter; a file containing only `\id`; an empty file; invalid UTF-8; the same file in NFC and NFD; deliberate zero-width joiners including one inside a marker name; `\vp` with non-ASCII digits; long conjunct chains; marks above and below on consecutive lines.

### 12.5 Composition

A synthetic composition sequence asserting exactly one `EditBatch` is emitted and the mirror checksum matches. Dispatchable without a real input method.
