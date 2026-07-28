# ADR-001 — Parser: adopt `usfm3` behind a facade

**Status:** Accepted
**Supersedes:** an earlier decision to hand-write the parser

## Context

We need a parser that is error-tolerant, produces accurate source spans, supports incremental reparse, never crashes on hostile input, and compiles to `wasm32-unknown-unknown` ([ADR-002](002-wasm-or-native.md)). It is the largest piece of work in the project and the one everything depends on.

**What is hard about USFM is not the grammar.** Tokenizing `\marker content` is easy. The difficulty is above it: which markers close explicitly versus implicitly; when the `\+` nesting prefix is required; which attributes are legal where and what the default is; whether `\v 3` after `\v 1-2` is a duplicate; whether a milestone ever gets its closing partner. Each is a marker-table lookup plus a semantic pass, and none is expressible in a context-free grammar.

Three properties resist grammar-based approaches specifically:

- **Custom `\z` markers are open-ended by design**, and real files lean on them — unfoldingWord's alignment uses `\zaln-s`/`\zaln-e` throughout. No grammar can know whether `\zfoo` is a paragraph marker, a character marker, or a milestone; it is inferred from usage at runtime.
- **Milestones are not tree-shaped.** `\qt-s\*` … `\qt-e\*` deliberately spans paragraph and verse boundaries — that is why milestones exist. Overlapping ranges cannot be nesting, so pairing is a flat post-pass whatever parses the tokens.
- **Error recovery is not error reporting.** We need *"unclosed `\bd` opened at line 12, column 4."* A generated parser's `ERROR` node means "something went wrong near here" and does not know what a `\bd` is.

## Options

### A — `tree-sitter-usfm3` (rejected)

| | |
|---|---|
| Version | `3.0.0-alpha.2` — no stable release, 2 releases ever |
| Recent downloads | ~54 |
| Composition | ~207k lines of generated C, 29 lines of Rust binding |

Compiling that plus the tree-sitter runtime to `wasm32-unknown-unknown` — no libc, against a runtime expecting `malloc` and `string.h` — is achievable with clang and a shim, but fiddly, adds a C toolchain across four build targets, and produces no USFM understanding. Using `web-tree-sitter` on the web and the Rust crate on desktop would reintroduce exactly the divergence [ADR-002](002-wasm-or-native.md) exists to eliminate.

### B — hand-written recursive descent (considered)

Since the marker table, nesting rules, milestone pairing, and diagnostics get written regardless, only the lexer and parser skeleton — roughly 2,100 of ~4,000 lines — is genuinely avoidable. Incremental reparse is unusually easy here because `\c` is a hard synchronization point, so hand-rolling it is ~200 lines rather than a research project.

That argument still holds. It means the avoidable two weeks are now *free*, not that they were never worth saving.

### C — `usfm3` (chosen)

[`usfm3`](https://crates.io/crates/usfm3) by James Cuénod. MIT, 9,492 lines of Rust, four dependencies (`logos`, `quick-xml`, `serde`, `serde_json`), all mature and wasm-safe. Ships Rust, CLI, PyPI, and **npm/WASM** bindings.

Its README states the design intent:

> The public API is staged: `tokenize → parse_cst → parse_ast / lower_cst → serialize`. That lets **editor-style integrations stay on the cheap token/CST path**, while AST-dependent work like USJ, USX, USFM, vref, and diagnostics is only paid for when requested.

and calls the CST *"the preferred CST/LSP path."* **This is a parser designed for an editor**, and the staging maps onto our tiering directly ([ARCHITECTURE §8.1](../ARCHITECTURE.md#81-facade)). The laziness is real — `ParsedDocument` uses `OnceCell` per stage.

Every objection to Option A is absent: pure Rust, four clean dependencies, WASM already demonstrated by the crate's own npm package.

## Decision

**`easy-usfm-core` wraps `usfm3`, pinned to an exact version, exposing our own API.**

**The round-trip risk does not apply.** The usual worry when adopting a parser is byte fidelity, and `usfm3`'s serializer works from the AST and is lossy by construction. We never call it — [ADR-003](003-source-authoritative.md) makes the buffer authoritative and saving writes the buffer. Byte-exactness is a property of our architecture, not of the parser.

Our requirements therefore reduce to four measurable properties: accurate spans, genuine error tolerance, no panics, adequate speed. Each is asserted continuously rather than assumed — the offset property tests, the three-way differential oracle, and the fuzz target (P0.3, P0.10, P0.11 in the [ROADMAP](../ROADMAP.md)) between them cover all four, and they keep covering them as `usfm3` is updated.

## Consequences

**What we still build** — roughly 1,400 lines against ~4,000 for Option B: incremental chapter-chunked session (~300), Char16 offset mapping (~200), diagnostic codes and version-derived severity (~400), normalization index (~250), verse range model (~150), grapheme segmentation (~100).

**Known friction.**

- **`source_map` is a parallel tree**, not spans on nodes — *"AST nodes do not carry spans."* Click-to-source and diagnostic placement walk two trees in lockstep, paired by position at each level, which is what the crate's own USJ serializer does. `to_usj(UsjOptions { include_spans: true })` is an alternative path.
- **Text leaves have no source location whatever.** Confirmed against 0.2.1 during P0.2: `Node::Text` and `Node::OptBreak` are recorded as `SourceNode::leaf()` — no span *and* no `anchor_cst`. The parallel tree is populated for structural nodes only. This is worse than "spans live elsewhere" and it lands on P3.6, which is the feature that needs text offsets most. Text spans have to come from the CST, which is source-backed and lossless; P0.4 owns that path. Our `Node::span` is `Option` so the gap is visible rather than papered over with a zero span.
- **Unrecognized marker names are leaked, permanently, per call.** `MarkerName::parse` interns via `Box::leak` for anything not in its table. Acceptable for a one-shot CLI; not for an editor that reparses as you type, where each keystroke of `\zaln-s` leaks a fresh prefix — and `\z` markers are pervasive in exactly the alignment-bearing files we care about. The facade owns its own marker strings so nothing above inherits it, but the leak is inside `usfm3` and grows with the session. **First thing to raise upstream**, and a reason P0.4's session design cannot simply reparse on every keystroke.
- **`parse()` copies the source** — `input.to_string()`, a 2 MB allocation per call. Use `parse_owned`; incremental reparse makes it largely moot.
- **No feature flags**, so `quick-xml` and the USX serializer compile into the WASM bundle unconditionally. A small upstream PR would fix this.
- **Documentation coverage is 32.6 %**, so several behavioural questions are answerable only by reading the source.

**The maturity risk.** Five months old at v0.2.1, one maintainer, 6 stars, 461 downloads, with an explicit notice that breaking changes will be made at the author's discretion before 1.0. Honest, and a real hazard for a dependency at the centre of the product. Four controls:

1. **Facade** — `easy-usfm-core` exposes our types. **Nothing above it knows `usfm3` exists.** Worth doing on its own merits, and it makes the dependency swappable.
2. **Exact pin** — `usfm3 = "=0.2.1"`, `Cargo.lock` vendored, updates taken deliberately with the corpus suite as gate.
3. **Fork is cheap** — MIT, 9,492 lines, four dependencies, no build exotica. A bad afternoon, not a crisis; contrast 207k lines of generated C.
4. **Become a visible user** — one open issue means a serious downstream consumer is likely welcomed. The author is already contemplating LSP integration, and incremental reparse is plausibly something he wants upstream. This converts single-maintainer risk into collaboration.

**Testing.** Having `usfm3` as a dependency makes the differential oracle three-way at almost no cost ([ARCHITECTURE §12.1](../ARCHITECTURE.md#121-three-way-differential-oracle)).

**Revisiting.** The parser sits behind the facade. Swapping implementations — a fork, Option B, or a future stable Option A — changes nothing above that boundary.

## References

[`usfm3` crate](https://crates.io/crates/usfm3) · [jcuenod/usfm3](https://github.com/jcuenod/usfm3) · [API docs](https://docs.rs/usfm3/latest/usfm3/) · [tree-sitter-usfm3](https://crates.io/crates/tree-sitter-usfm3) (rejected) · [usfm-grammar](https://github.com/Bridgeconn/usfm-grammar) (differential oracle)
