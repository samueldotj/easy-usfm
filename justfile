# easy-usfm task runner — https://github.com/casey/just
#
# just is optional. Every recipe below is a single command you can run
# directly; the equivalents are listed in corpus/README.md.
#
#   Install:  winget install Casey.Just     (Windows)
#             brew install just             (macOS)
#             cargo install just            (anywhere with Rust)

# Recipes are single commands, so they work under PowerShell without needing
# a POSIX shell on Windows.
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# Windows generally has no `python3`; the py launcher is the reliable entry
# point there. Override if your setup differs:  just py=python corpus-verify
py := if os_family() == "windows" { "py -3" } else { "python3" }

default:
    @just --list

# ---------------------------------------------------------------- corpus ---

# Download the extended tier from eBible.org (redistributable translations only)
corpus-fetch *ARGS:
    {{py}} tools/corpus/fetch.py {{ARGS}}

# List redistributable translations without downloading anything
corpus-list:
    {{py}} tools/corpus/fetch.py --list

# Choose the committed core tier from the fetched extended tier
corpus-select target="200":
    {{py}} tools/corpus/select.py corpus/extended --target {{target}} --copy-to corpus/core

# Verify the committed corpus: hashes, provenance, coverage. Runs in CI.
corpus-verify:
    {{py}} tools/corpus/verify.py

# Report scripts, features, and encoding traits per file
corpus-classify path="corpus/core":
    {{py}} tools/corpus/classify.py {{path}}

# Summarise coverage and list anything missing
corpus-coverage path="corpus/core":
    {{py}} tools/corpus/classify.py {{path}} --coverage

# Self-test the corpus tooling (no network)
corpus-test:
    {{py}} tools/corpus/test_tooling.py

# Rebuild the corpus from scratch: fetch, select, verify
corpus-rebuild: corpus-fetch corpus-select corpus-verify
