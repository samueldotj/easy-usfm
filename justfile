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

default:
    @just --list

# ---------------------------------------------------------------- corpus ---
#
# The corpus tooling is `cargo xtask`, so it needs no interpreter beyond the
# Rust toolchain the project already requires. The .cargo alias makes it work
# from a bare checkout with nothing else installed.

# Download the extended tier: eBible.org plus the curated repositories
corpus-fetch *ARGS:
    cargo xtask corpus fetch {{ARGS}}

# List redistributable sources without downloading anything
corpus-list:
    cargo xtask corpus fetch --list

# Choose the committed core tier from the fetched extended tier
corpus-select target="200":
    cargo xtask corpus select --target {{target}}

# Verify the committed corpus: hashes, provenance, coverage. Runs in CI.
corpus-verify:
    cargo xtask corpus verify

# Report scripts, features, and encoding traits per file
corpus-classify path="corpus/core":
    cargo xtask corpus classify {{path}}

# Summarise coverage and list anything missing
corpus-coverage path="corpus/core":
    cargo xtask corpus classify {{path}} --coverage

# Self-test the corpus tooling (no network)
corpus-test:
    cargo test --package xtask

# Rebuild the corpus from scratch: fetch, select, verify
corpus-rebuild: corpus-fetch corpus-select corpus-verify

# ---------------------------------------------------------------- engine ---

# Everything CI runs, in the order it runs it
check: fmt-check lint test wasm

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

# The engine has to compile for the target it actually ships on
wasm:
    cargo build --package easy-usfm-core --target wasm32-unknown-unknown
