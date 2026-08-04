# ADR-004 — The document model is USJ

**Status:** Accepted

## Context

The preview, diagnostics, navigation, and formatting commands all read from a parsed representation, and that representation needs a shape. The default instinct is to design one: a tree of node types matching exactly what the preview renders, and nothing more.

USFM 3.1 changed the calculus. The specification now defines **one content model with three equivalent expressions** — USFM (backslash markers), USX (XML), and USJ (JSON). Not three formats with converters between them: three serializations of a single published, versioned model.

## Decision

**The document tree is the USJ content model**, extended with source spans and a variant for content the parser could not classify. Struct in [ARCHITECTURE §5](../ARCHITECTURE.md#5-document-model).

## Rationale

**Validation checks against a specification, not our opinions.** A bespoke model makes every structural question a judgement call — is `\cl` a child of the chapter or a sibling? Does `\esb` nest inside a paragraph? Each gets answered once, informally, by whoever is writing that part of the preview, and the answers drift. Adopting the published model means those questions have documented answers, and disagreement with them is a bug rather than a preference. When UBS revises the specification, the change arrives as a model delta to track rather than a reinterpretation exercise.

**It makes the differential oracle possible** — the practical payoff, and larger than it sounds. Two independent mature implementations, `usfm3` and `usfm-grammar`, both emit USJ natively. Because our tree is USJ-shaped, all three can be diffed **structurally** rather than by comparing rendered output or eyeballing behaviour ([ARCHITECTURE §12.1](../ARCHITECTURE.md#121-three-way-differential-oracle)). A bespoke model would need a translation layer for the comparison, which would itself need testing, which defeats the purpose.

**It makes an excluded feature cheap to un-exclude.** USX and USJ export are out of scope for v1.0, and with a USJ-shaped tree they are a serializer over a structure already in the right shape — a day's work whenever wanted. The point is not to build it now; the point is that excluding it costs nothing later, which makes the exclusion easy to defend.

**It aligns with the chosen parser at no cost.** `usfm3` produces USJ natively ([ADR-001](001-parser.md)); a bespoke model would need a conversion layer in the facade, with more code and more places for spans to be mangled. This is a consequence rather than a reason — the validation and oracle arguments stand alone, and would hold with a hand-written parser.

## Consequences

**Two extensions to the published model.**

*Source spans.* USJ has no notion of where a node came from. We add `span`, and it is load-bearing — every feature connecting preview to text depends on it ([ADR-003](003-source-authoritative.md)). `usfm3` keeps spans in a parallel `source_map` tree rather than on nodes, so the facade reconciles the two; this is the main API friction noted in ADR-001.

*A `raw` variant.* The published model describes valid documents. We must also hold invalid ones, because malformed content has to survive editing and saving. `Node::raw` carries a verbatim source span for anything unclassifiable, and the preview renders it as an inline placeholder.

Both are additions, not modifications — a node with `span` and `raw` stripped is valid USJ.

**The model is richer than the preview needs.** USJ models constructs v1.0 does not render specially, such as peripherals and some sidebar structures. Carrying them costs a little memory and some match arms falling through to a generic renderer. Accepted deliberately: an unmodelled construct is one we would silently drop, and dropping content is exactly what [ADR-003](003-source-authoritative.md) exists to prevent. Better to model it and render it plainly than not to see it.

**Naming follows the specification.** Node kinds, marker classes, and attribute names use the specification's vocabulary rather than invented synonyms — slightly less idiomatic Rust in places, considerably easier to check an implementation against the documentation.

## Alternatives considered

**Bespoke preview-shaped tree.** Smallest and simplest, and it fails the validation and oracle arguments. It would also have made the parser decision harder, since every candidate emits USJ or USX and we would be converting away from that.

**USX (XML) as the internal model.** Same content model, worse ergonomics in Rust — attribute-versus-element distinctions carrying no meaning for us, and a heavier serialization story. USJ maps onto Rust structs directly.

**No tree at all, rendering straight from tokens.** Viable for syntax highlighting, which does use the token path, and inadequate for a semantic preview with notes, nesting, and milestones.
