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

**Python 3.9 or later. Nothing else** — no pip install, no virtualenv. The
tooling is standard library only, because `verify.py` runs in CI on every push
and a corpus check that needs its own dependency tree is a corpus check that
breaks.

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
| Download the extended tier | `just corpus-fetch` | `python3 tools/corpus/fetch.py` |
| Choose ~200 files, write the manifest | `just corpus-select` | `python3 tools/corpus/select.py corpus/extended --target 200 --copy-to corpus/core` |
| Verify — the CI gate | `just corpus-verify` | `python3 tools/corpus/verify.py` |
| All three | `just corpus-rebuild` | run the above in order |

On Windows use `py -3` rather than `python3`. The justfile already does this;
override with `just py=python corpus-verify` if your setup differs.

Inspect before committing:

| | With `just` | Directly |
|---|---|---|
| What the candidate pool covers | `just corpus-coverage corpus/extended` | `python3 tools/corpus/classify.py corpus/extended --coverage` |
| Per-file scripts, features, traits | `just corpus-classify` | `python3 tools/corpus/classify.py corpus/core` |
| Redistributable translations available | `just corpus-list` | `python3 tools/corpus/fetch.py --list` |
| Self-test the tooling (no network) | `just corpus-test` | `python3 tools/corpus/test_tooling.py` |

Start with `--dry-run` if you want to see what would be downloaded before
anything is:

```sh
python3 tools/corpus/fetch.py --dry-run
```

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

Enforced by `verify.py`; defined in `tools/corpus/usfm_features.py`.

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
`source` and `copyright`, and run `just corpus-verify`. Hand-authored
pathological cases belong in `tests/pathological/` instead — they are not
published Scripture and should not claim to be.
