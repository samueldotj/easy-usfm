# easy-usfm task runner — https://github.com/casey/just

default:
    @just --list

# ---------------------------------------------------------------- corpus ---

# Download the extended tier from eBible.org (redistributable translations only)
corpus-fetch *ARGS:
    python3 tools/corpus/fetch.py {{ARGS}}

# List redistributable translations without downloading anything
corpus-list:
    python3 tools/corpus/fetch.py --list

# Choose the committed core tier from the fetched extended tier
corpus-select target="200":
    python3 tools/corpus/select.py corpus/extended \
        --target {{target}} --copy-to corpus/core

# Verify the committed corpus: hashes, provenance, coverage. Runs in CI.
corpus-verify:
    python3 tools/corpus/verify.py

# Report scripts, features, and encoding traits per file
corpus-classify path="corpus/core":
    python3 tools/corpus/classify.py {{path}}

# Summarise coverage and list anything missing
corpus-coverage path="corpus/core":
    python3 tools/corpus/classify.py {{path}} --coverage

# Rebuild the corpus from scratch: fetch, select, verify
corpus-rebuild: corpus-fetch corpus-select corpus-verify
