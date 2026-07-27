# easy-usfm

An editor for individual USFM Scripture files, for desktop and the web.

A USFM source editor beside a live Scripture preview, with native menus, dialogs, and keyboard conventions. Works entirely offline. Handles complex scripts correctly.

**The USFM source text is the authoritative document.** Everything else is derived and disposable.

---

## Status

Design complete, no implementation yet. Work starts at M0 — the parser layer over [`usfm3`](https://crates.io/crates/usfm3) ([ADR-001](docs/adr/001-parser.md)).

| | Milestone | What it means |
|---|---|---|
| **M0** | Foundations | The engine reads USFM correctly. No interface. |
| **M1** | An editor you can trust | Open, edit, and save any USFM file without risk of losing it. |
| **M2** | It understands USFM | Highlighting, error reporting, verse navigation, marker help. |
| **M3** | You can see your Scripture | Live preview and printing — first build worth showing anyone. |
| **M4** | Reliable on a real machine | Survives crashes, cloud folders, files changed underneath you. |
| **M5** | Runs in a browser | Installable web app, works offline. |
| **M6** | Version 1.0 | Signed installers, accessible, verified across scripts. |

61 work items sized S to XL — see [ROADMAP](docs/ROADMAP.md).

## Stack

```text
Desktop shell     Tauri 2 + Rust  (file I/O, dialogs, watching, recovery)
Frontend          Svelte + Vite + TypeScript  (no SvelteKit)
Editor            CodeMirror 6
USFM engine       Rust → WebAssembly, in a Web Worker, on all targets
USFM parser       usfm3 crate, behind our own facade
USFM version      3.2
Document model    USJ
```

## Planned layout

```text
crates/
├── easy-usfm-core/   facade, incremental session, offsets, diagnostics, indexes
├── easy-usfm-wasm/   wasm-bindgen surface, worker protocol
└── easy-usfm-tauri/  file access, atomic save, recovery, watching
src/                  Svelte frontend
docs/                 design documentation
```

---

## Documentation

| Document | Covers |
|---|---|
| [PRODUCT](docs/PRODUCT.md) | Goals, scope, interface, editor and preview behaviour, printing, diagnostics, acceptance criteria |
| [ARCHITECTURE](docs/ARCHITECTURE.md) | Layering, engine, parser facade, incremental parsing, delta protocol, performance, engine tests |
| [FILE-FIDELITY](docs/FILE-FIDELITY.md) | Encoding, line endings, atomic save, recovery, external changes |
| [UNICODE](docs/UNICODE.md) | Coordinate spaces, graphemes, normalization, IME, fonts, text direction |
| [SECURITY](docs/SECURITY.md) | CSP, links, figures, capabilities, logging |
| [ROADMAP](docs/ROADMAP.md) | Phase 0 gate through release |

**Decision records** — the *why*, with rejected options kept on record:

[001 Parser](docs/adr/001-parser.md) · [002 WASM or native](docs/adr/002-wasm-or-native.md) · [003 Source authoritative](docs/adr/003-source-authoritative.md) · [004 USJ model](docs/adr/004-usj-model.md) · [005 Save strategy](docs/adr/005-save-strategy.md)

### Reading order

- **Implementing?** [ADR-003](docs/adr/003-source-authoritative.md) first — it is load-bearing and the rest follows from it. Then [ARCHITECTURE](docs/ARCHITECTURE.md).
- **On the parser?** [ADR-001](docs/adr/001-parser.md), then Phase 0 in [ROADMAP](docs/ROADMAP.md).
- **On save or recovery?** [FILE-FIDELITY](docs/FILE-FIDELITY.md) and [ADR-005](docs/adr/005-save-strategy.md).
- **Touching text offsets?** [UNICODE §1](docs/UNICODE.md#1-three-coordinate-spaces), before writing code. Conflating the three coordinate spaces is the most likely serious bug here, and ASCII fixtures cannot detect it.

---

MIT — see [LICENSE](LICENSE).
