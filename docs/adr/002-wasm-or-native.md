# ADR-002 — One WASM engine on all targets

**Status:** Accepted

## Context

The application ships as a Tauri 2 desktop app and a hosted web app sharing one Svelte frontend. The USFM engine is Rust. There are two ways to run it.

**A — dual path.** Compile natively for desktop and call through Tauri IPC; compile to WebAssembly for the browser. The conventional Tauri architecture.

**B — WASM everywhere.** Compile to `wasm32-unknown-unknown` once and run that artifact in a Web Worker on both targets, leaving Tauri's Rust side responsible only for file I/O.

Option A's stated goal was preventing the two platforms from interpreting USFM differently, by sharing Rust source.

## Decision

**B.** The engine ships as WebAssembly and the same artifact runs on desktop and web. Native Rust is retained exclusively for what browsers cannot do: dialogs, reading, atomic writing, watching, recovery, OS integration.

## Rationale

**Option A does not achieve its goal.** Sharing source is not sharing behaviour. It ships two compilation targets, **two adapter layers** (Tauri IPC and `wasm-bindgen`), **two offset-conversion paths** — where the most dangerous class of bug in this application lives ([UNICODE §1](../UNICODE.md#1-three-coordinate-spaces)) — two error paths, two serialization formats, two test surfaces. The shared-source argument covers the parser and leaves everything around it duplicated. Divergence rarely arises in the parser; it arises in the glue.

**The performance objection does not survive measurement.** WASM runs roughly 1.2–2× slower than native for this workload, which is immaterial against our budgets for two reasons. The bottleneck is DOM rendering — a 2 MB book produces 10⁴–10⁵ preview nodes, and rebuilding that dominates any plausible parse time. And the IPC hop costs more than the penalty saves: Option A must move the text or a parse result across Tauri's boundary, which serializes. Incremental parsing reduces the per-edit parse to a single chapter (~10 KB), at which point the WASM penalty is microseconds.

**What B buys.** The `invoke("parse", …)` surface disappears with its serialization cost and error paths. The web build is exercised by every desktop test run — under A it is validated late and separately, which is how it ends up broken in Phase 5; under B it is functional from Phase 2. `easy-usfm-core` develops and tests with no Tauri toolchain, so parser contributors need Rust and nothing else. And "desktop and web run the same engine" becomes a build-hash comparison rather than an assertion.

**Interaction with ADR-001.** This constrains the parser choice: `tree-sitter-usfm3` would need ~207k lines of generated C and a libc shim on wasm32, while `usfm3` is pure Rust and already ships a working WASM package. Option A would have made the tree-sitter route easier by letting desktop use native C — which is a reason to be suspicious of the dual path, not to adopt it. It would have let a wasm-hostile dependency in through a side door.

## Consequences

**Positive.** Single code path and single test surface for USFM interpretation. Web validated continuously. Phase 5 shrinks to service worker, File System Access API, and IndexedDB recovery. Parser development decoupled from desktop tooling.

**Negative, accepted.**

- **A CSP allowance.** `script-src 'self' 'wasm-unsafe-eval'` is needed to instantiate the module. It does not permit JavaScript `eval`, but it is a relaxation ([SECURITY §4](../SECURITY.md#4-content-security-policy)).
- **Bundle size matters now.** `usfm3` has no feature flags, so `quick-xml` and the USX serializer compile in unconditionally. Under A this would have been desktop-only dead weight.
- **Debugging is harder.** Rust stack traces through WASM are worse than native. Mitigated by the `easy-usfm-core` CLI, which runs natively and is where parser bugs actually get diagnosed.
- **Native-only capabilities are given up** — threads, memory mapping, native SIMD. None is needed; the engine is single-threaded by design and runs in a worker.

**Structural requirement.** Because the engine cannot reach the filesystem on either target, `easy-usfm-core` **must** be pure computation over text. Enforced by the crate layout and by keeping it separable, with no Tauri dependency available to accidentally reach for.

## Revisiting

If profiling ever puts WASM parse time on the critical path — which would require the DOM bottleneck to disappear first — the facade in [ADR-001](001-parser.md) makes a native path addable for desktop without touching anything above `easy-usfm-core`. It would reintroduce the dual-adapter problem, so treat it as a last resort rather than an optimization to reach for.
