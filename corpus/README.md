# Test corpus

Real USFM files. Fixtures do not find parser bugs; published Scripture does.

Two tiers, for the reasons in [ARCHITECTURE §12.4](../docs/ARCHITECTURE.md#124-corpus).

| | Where | Committed | Runs |
|---|---|---|---|
| **Core** | `corpus/core/` | yes, with `manifest.toml` | every push |
| **Extended** | `corpus/extended/` | no, gitignored | nightly |

The core tier is chosen for coverage, not sampled: every required script, every
USFM feature class, and every encoding trait must appear in it. Around 200 files
and 20 MB — small enough that cloning stays pleasant, broad enough that a change
to note handling cannot pass CI without a file that has notes.

## Requirements

**The Rust toolchain. Nothing else** — the tooling is `cargo xtask`, so it runs
in the same toolchain the engine needs and a contributor installs one thing
rather than two. The `.cargo` alias makes it work from a bare checkout.

If you have nothing installed yet, the bootstrap script handles it:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\bootstrap.ps1 -Minimal   # Windows
```
```sh
scripts/bootstrap.sh --minimal                                            # macOS / Linux
```

[`just`](https://github.com/casey/just) is **optional** — a task runner that
saves typing. Every recipe is a single command you can run directly, listed
below.

## Building it

| | With `just` | Directly |
|---|---|---|
| Download the extended tier | `just corpus-fetch` | `cargo xtask corpus fetch` |
| Choose ~200 files, write the manifest | `just corpus-select` | `cargo xtask corpus select --target 200` |
| Verify — the CI gate | `just corpus-verify` | `cargo xtask corpus verify` |
| All three | `just corpus-rebuild` | run the above in order |

Inspect before committing:

| | With `just` | Directly |
|---|---|---|
| What the candidate pool covers | `just corpus-coverage corpus/extended` | `cargo xtask corpus classify corpus/extended --coverage` |
| Per-file scripts, features, traits | `just corpus-classify` | `cargo xtask corpus classify corpus/core` |
| Redistributable sources available | `just corpus-list` | `cargo xtask corpus fetch --list` |
| Self-test the tooling (no network) | `just corpus-test` | `cargo test --package xtask` |

Start with `--dry-run` if you want to see what would be downloaded before
anything is:

```sh
cargo xtask corpus fetch --dry-run
```

## Where the files come from

Two kinds of source, which `select` and `verify` treat identically — they
differ only in how the licence evidence is gathered.

**eBible.org**, for breadth. Hundreds of translations behind one catalogue with
a machine-readable `Redistributable` flag, which is what makes the licence
question evidence rather than interpretation.

**Curated repositories**, listed by hand in `xtask/src/github.rs` with their
licence basis recorded alongside, and pinned to a commit rather than a branch
so the pool cannot drift underneath a failing test:

| Source | Text | Terms |
|---|---|---|
| [FreeBiblesIndia/Tamil_Bible](https://github.com/FreeBiblesIndia/Tamil_Bible) | Tamil | CC BY-SA 4.0 |
| [FreeBiblesIndia/Hindi_Bible](https://github.com/FreeBiblesIndia/Hindi_Bible) | Hindi (Devanagari) | CC BY-SA 4.0 |
| [dharmatech/bsb-usfm](https://github.com/dharmatech/bsb-usfm) | Berean Standard Bible | Public domain |

The two Creative Commons texts require **attribution** — *"Original work
available at http://www.freebiblesindia.in"* — and carry a **ShareAlike**
condition. They are test data rather than part of any released artefact, and
the manifest records the licence line for every file, but the repository is MIT
and now carries CC BY-SA content, which is worth knowing before any of it is
copied elsewhere.

Curated sources are guaranteed a share of the committed tier
(`--per-source`, default 10). Without that floor the selector, which spreads by
script rarity, drops a hand-picked Latin source entirely in favour of the dozens
eBible supplies — silently discarding a deliberate decision.

## Licensing

**Only translations eBible.org marks `Redistributable` are ever fetched**, and
`verify.py` fails the build if a manifest entry says otherwise. That flag is
published by the distributor per translation, which turns the licence question
from interpretation into evidence-gathering — the manifest records the copyright
line and the flag for every committed file, so the basis for including it is
auditable without re-reading anything.

Two limits worth stating plainly:

- The flag is eBible's assertion, not legal advice. For a file you intend to
  redistribute in a released artefact rather than a test corpus, check the
  publisher's own terms.
- The extended tier is **not** committed, so its files only need to be readable
  where they are published. That is deliberate: it keeps the redistribution
  question confined to the ~200 files that actually enter the repository.

## Coverage requirements

Enforced by `cargo xtask corpus verify`; defined in `xtask/src/features.rs`.

**Scripts** — Latin, Greek, Cyrillic, Hebrew, Arabic, Devanagari, Tamil,
Bengali, Thai, Khmer, Myanmar, Han. Chosen to exercise combining marks,
conjunct formation, visual reordering, right-to-left, and the absence of word
spacing.

**Feature classes** — notes, poetry, lists, tables, milestones, attributes,
sidebars, figures, introductions, peripherals, custom `\z` markers, titles,
character styles, alternate numbering, verse ranges, nested markers.

**Encoding traits** — BOM, LF, CRLF, mixed line endings, missing final newline,
non-NFC normalization, zero-width joiners.

The trait list is why the corpus cannot be all-Latin and all-tidy: several
[FILE-FIDELITY](../docs/FILE-FIDELITY.md) guarantees are only tested by files
that are genuinely messy.

### Six goals nothing published covers

`verify` currently runs with `--skip-coverage` in CI, and the reason is not
that the gate is inconvenient. Across a pool of 2,730 files from 53 sources,
six goals have no candidate at all:

| Missing | Why |
|---|---|
| `milestones`, `sidebars`, `custom_z` | Rare outside alignment-bearing texts. unfoldingWord's `\zaln-s` data is the obvious source, and is not yet listed. |
| `bom`, `crlf`, `mixed_eol` | **Published Scripture never carries them.** Distributors normalise line endings and strip byte-order marks before publishing. |

The second row is the awkward one, because those three traits are exactly what
[FILE-FIDELITY](../docs/FILE-FIDELITY.md) exists to protect. No amount of
additional real translations will supply them — a wider net cannot catch what
nobody publishes. They have to be synthesised: either re-encoded variants of
files already committed here, or fixtures that live in `tests/pathological/`
and are exercised separately, in which case the trait list belongs there rather
than in this corpus's coverage requirements.

That decision is open. Until it is made, coverage is measured and reported but
not enforced, so the gap stays visible rather than being quietly deleted from
the requirements.

## Layout

```text
corpus/
├── README.md          this file
├── manifest.toml      generated; one [[file]] per committed file
├── core/              ~200 committed files
├── extended/          fetched, gitignored
└── .catalog.csv       cached eBible catalog, gitignored
```

## Manifest

Each entry records what the file is, where it came from, and why it is here:

```toml
[[file]]
path = "core/41MATengwebp.usfm"
sha256 = "…"
bytes = 187432
translation = "engwebp"
source = "https://ebible.org/Scriptures/engwebp_usfm.zip"
language = "English"
script_declared = "Latin"
direction = "ltr"
copyright = "Public Domain"
redistributable = "True"
scripts = ["Latin"]
features = ["char_styles", "notes", "poetry", "titles"]
traits = ["lf"]
```

`sha256` is the point of the file: it makes drift detectable. If a corpus file
changes silently, every downstream test failure becomes ambiguous.

## Adding a file by hand

Rare, but sometimes a construct is not represented in anything eBible
distributes. Put it in `corpus/core/`, add a `[[file]]` entry with a real
`source` and `copyright`, and run `cargo xtask corpus verify`. Hand-authored
pathological cases belong in `tests/pathological/` instead — they are not
published Scripture and should not claim to be.
