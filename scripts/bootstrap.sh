#!/usr/bin/env bash
# Install the toolchain easy-usfm needs, on macOS and Linux.
#
# Idempotent: anything already present is left alone.
#
#   scripts/bootstrap.sh              everything
#   scripts/bootstrap.sh --minimal    Rust only (engine and corpus tooling)

set -uo pipefail

MINIMAL=0
SKIP_RUST=0
SKIP_NODE=0
SKIP_JUST=0
for arg in "$@"; do
  case "$arg" in
    --minimal)   MINIMAL=1 ;;
    --skip-rust) SKIP_RUST=1 ;;
    --skip-node) SKIP_NODE=1 ;;
    --skip-just) SKIP_JUST=1 ;;
    -h|--help)   sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

C_STEP='\033[36m'; C_OK='\033[32m'; C_WARN='\033[33m'; C_ERR='\033[31m'
C_DIM='\033[90m'; C_OFF='\033[0m'
step() { printf "\n${C_STEP}=> %s${C_OFF}\n" "$1"; }
ok()   { printf "   ${C_OK}ok    %s${C_OFF}\n" "$1"; }
have() { printf "   ${C_DIM}have  %s${C_OFF}\n" "$1"; }
warn() { printf "   ${C_WARN}warn  %s${C_OFF}\n" "$1"; }
err()  { printf "   ${C_ERR}FAIL  %s${C_OFF}\n" "$1"; }
tool() { command -v "$1" >/dev/null 2>&1; }

# ---- package manager -------------------------------------------------------

step "Detecting package manager"
if tool brew;    then PM=brew;   INSTALL="brew install"
elif tool apt-get; then PM=apt;  INSTALL="sudo apt-get install -y"
elif tool dnf;   then PM=dnf;    INSTALL="sudo dnf install -y"
elif tool pacman; then PM=pacman; INSTALL="sudo pacman -S --noconfirm"
elif tool zypper; then PM=zypper; INSTALL="sudo zypper install -y"
else
  err "no supported package manager found (brew, apt, dnf, pacman, zypper)"
  echo "   rustup installs without one; see https://rustup.rs"
  PM=none; INSTALL=""
fi
[ "$PM" != none ] && ok "$PM"

pkg() {  # pkg <probe> <label> <brew-name> <linux-name>
  local probe="$1" label="$2" bname="$3" lname="$4"
  if tool "$probe"; then have "$label"; return 0; fi
  if [ "$PM" = none ]; then warn "$label missing and no package manager"; return 1; fi
  echo "   installing $label…"
  local name="$lname"; [ "$PM" = brew ] && name="$bname"
  if $INSTALL "$name" >/dev/null 2>&1; then
    tool "$probe" && { ok "$label"; return 0; }
  fi
  warn "$label install did not succeed"
  return 1
}

# ---- Rust — required -------------------------------------------------------
#
# The corpus tooling is `cargo xtask`, so Rust is the only interpreter the
# project needs. Nothing here requires Python.

if [ "$SKIP_RUST" -eq 0 ]; then
  step "Rust toolchain (engine and corpus tooling)"
  if tool rustup || tool cargo; then
    have "rust ($(rustc --version 2>/dev/null || echo installed))"
  else
    echo "   installing rustup…"
    if curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y >/dev/null 2>&1; then
      # shellcheck disable=SC1091
      [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
      tool cargo && ok "rustup"
    else
      warn "rustup install failed — see https://rustup.rs"
    fi
  fi
  if tool rustup; then
    rustup target add wasm32-unknown-unknown >/dev/null 2>&1 && ok "wasm32-unknown-unknown"
  fi
fi

# ---- optional toolchain ----------------------------------------------------

if [ "$MINIMAL" -eq 0 ]; then

  if [ "$SKIP_JUST" -eq 0 ]; then
    step "just (task runner — optional)"
    if tool just; then
      have "just"
    elif ! pkg just "just" just just; then
      # just is a Rust program, so cargo is a dependable fallback.
      if tool cargo; then
        echo "   installing just via cargo…"
        cargo install just >/dev/null 2>&1 && tool just && ok "just (via cargo)"
      fi
    fi
  fi

  if [ "$SKIP_NODE" -eq 0 ]; then
    step "Node.js (frontend, and the usfm-grammar test oracle)"
    pkg node "Node.js" node nodejs || true
  fi
fi

# ---- verify ----------------------------------------------------------------

step "Verifying"

for t in cargo git just node; do
  if tool "$t"; then
    ok "$(printf '%-7s' "$t") $($t --version 2>&1 | head -1)"
  elif [ "$MINIMAL" -eq 1 ] && { [ "$t" = just ] || [ "$t" = node ]; }; then
    have "$t (skipped: --minimal)"
  else
    warn "$t not found"
  fi
done

if tool rustup; then
  if rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
    ok "target  wasm32-unknown-unknown"
  else
    warn "wasm32-unknown-unknown not installed — the engine will not build for the web"
  fi
fi

# ---- smoke test ------------------------------------------------------------

if tool cargo; then
  step "Smoke test — corpus tooling self-test"
  cd "$(dirname "$0")/.." || exit 1
  if cargo test --package xtask; then ok "corpus tooling works"
  else err "self-test failed"; fi
fi

# ---- next ------------------------------------------------------------------

cat <<EOF

$(printf "${C_STEP}Next${C_OFF}")

   Check the engine builds:

       cargo test --workspace
       cargo build -p easy-usfm-core --target wasm32-unknown-unknown

   The committed corpus is already in the repository. To re-verify it, or to
   rebuild the fetched tier:

       cargo xtask corpus verify
       cargo xtask corpus fetch --dry-run    # see what would download
       cargo xtask corpus fetch              # download (~10 min)
       cargo xtask corpus select --target 200

   Or, with just installed:

       just check
       just corpus-rebuild

   See corpus/README.md for what the tiers are and why.
EOF
