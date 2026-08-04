# ADR-003 — The source text is authoritative

**Status:** Accepted

## Context

An editor with a live preview holds two representations: the text the user types, and the parsed structure the preview renders from. One must be authoritative.

**A — tree authoritative.** Parse on open, edit the tree, serialize on save. How word processors and most structured editors work. Structural operations are natural and the saved file is always well-formed.

**B — text authoritative.** The buffer is the document; the tree is derived, cached, and discarded freely. Saving writes the buffer.

USFM makes the choice unusually consequential, because files in circulation contain things a tree cannot faithfully hold:

- **Unknown and custom markers.** `\z` extensions are open-ended by design; unfoldingWord's alignment data uses `\zaln-s`/`\zaln-e` throughout.
- **Comments, remarks, and whitespace conventions** meaningful to the translation team and to no parser.
- **Temporarily malformed content.** Someone mid-edit has an unclosed `\bd`. That state must be savable.
- **Byte-level properties** — BOM, mixed line endings, normalization form — that no tree records.

## Decision

**The source text is authoritative. Easy USFM never serializes a document from its tree.**

Saving writes the buffer with the fidelity envelope reapplied ([FILE-FIDELITY §1](../FILE-FIDELITY.md#1-the-fidelity-envelope)). The tree, diagnostics, verse index, preview model, and source mappings are all derived, disposable, and reconstructible from the text alone.

```text
USFM source text  ← authoritative, byte-exact
       ↓
Tolerant parser  →  tree  →  diagnostics · indexes · preview model
```

The arrow points one way. There is no path from the tree back to the file.

## Rationale

**Preservation becomes structural, not aspirational.** Under A, preserving unknown markers requires the tree to model "content I did not understand" and the serializer to reproduce it byte-exactly — a promise the parser must keep on every construct, forever, including ones added to USFM after we ship. Under B it is not a promise at all: bytes we never touched are bytes we cannot damage. And "opening and saving an unchanged file produces identical bytes" is satisfied by not writing anything at all ([FILE-FIDELITY §1](../FILE-FIDELITY.md#1-the-fidelity-envelope)).

**Malformed states are representable.** A must answer "what does the tree look like while the user is halfway through typing `\bd`?" Every answer is either a lie about the document or a refusal to save. B has nothing to answer — the text is whatever it is, the parser reports diagnostics, saving works. Diagnostics never block saving because they are advisory observations about a document that exists independently of them.

**It removes a class of dependency risk** — the consequence that turned out to matter most. Adopting a third-party parser ([ADR-001](001-parser.md)) would normally require trusting its serializer to round-trip byte-exactly, and `usfm3`'s works from the AST and is lossy by construction. Under A that is disqualifying. Under B we never call it, and the requirements reduce to accurate spans, error tolerance, no panics, and speed — all measurable in a week. Byte-exactness is a property of our architecture, not of a dependency.

**The cost is real and acceptable.** A would make structural edits natural — "delete this verse," "renumber chapters," "convert 2.x `\fig` to attributes" — as tree operations. Under B each is a text edit computed from tree positions. For the operations v1.0 offers (marker wrapping, toggling, quick fixes) that is not meaningfully harder, and it composes correctly with undo because it goes through the same transaction system as typing. Large-scale restructuring would be more work, and is out of scope.

## Consequences

**Derived state must be cheap to rebuild.** Because the tree is disposable it gets rebuilt often, which forces incremental parsing to be genuinely incremental rather than an optimization, and forces stale results to be discarded rather than merged ([ARCHITECTURE §8](../ARCHITECTURE.md#8-parsing)).

**Source mappings are load-bearing.** Every feature connecting preview to text — click-to-source, diagnostic placement, verse navigation, formatting commands — depends on accurate spans. Under A these would be tree-internal; under B they cross a boundary and must be correct in a coordinate space the frontend understands, which is why [UNICODE §1](../UNICODE.md#1-three-coordinate-spaces) is type-enforced rather than conventional.

**Normalization must be handled without touching the buffer.** The buffer keeps the file's original form and never changes it, so search cannot compare raw bytes or an NFC query misses NFD text visibly on screen. Resolved by a parallel normalized index ([UNICODE §4](../UNICODE.md#4-normalization-versus-byte-fidelity)) — comparison normalizes, storage never does. This is a direct consequence of this ADR and the least obvious one.

**Explicit rewrites remain possible.** Nothing here forbids "Normalize line endings," "Normalize to NFC," or "Convert `\fig` to 3.x syntax." Those are ordinary edits: user-initiated, undoable, marking the document dirty. What is forbidden is any *automatic* rewrite.

## Revisiting

This is the load-bearing decision. [ADR-001](001-parser.md) and [ADR-005](005-save-strategy.md) both depend on it, and the preservation guarantees in [PRODUCT §4](../PRODUCT.md#4-usfm-handling) restate it as a user promise. Reversing it would mean rewriting the save path, the parser contract, the normalization strategy, and the acceptance criteria. Treat it as fixed.
