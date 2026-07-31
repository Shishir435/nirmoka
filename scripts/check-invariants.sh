#!/usr/bin/env bash
#
# Architecture invariants, enforced by grep rather than by good intentions.
#
# Each check encodes a rule from docs/architecture.md that is otherwise only a
# paragraph someone has to remember. Run by both the pre-push hook and CI from
# this one file, so the two can never drift apart.
#
# Usage: ./scripts/check-invariants.sh

set -uo pipefail

cd "$(dirname "$0")/.."

failed=0

fail() {
  printf '\033[31mFAIL\033[0m  %s\n' "$1" >&2
  failed=1
}

pass() {
  printf '\033[32mok\033[0m    %s\n' "$1"
}

# ---------------------------------------------------------------------------
# 1. core and adapter must not depend on a GUI framework.
#
# This is the boundary that makes the frontend replaceable. crates/cli exists
# to turn a violation into a build failure; this catches it a step earlier.
# ---------------------------------------------------------------------------
if grep -qE '^[[:space:]]*tauri' crates/core/Cargo.toml crates/adapter/Cargo.toml 2>/dev/null; then
  fail "core/adapter must not depend on tauri (see docs/adr/0005)"
else
  pass "core and adapter are GUI-framework-free"
fi

# ---------------------------------------------------------------------------
# 2. No platform conditionals in core.
#
# Matches the attribute form only and drops comment lines — the rule is
# documented inside core/src/lib.rs, which must not trip its own guard.
# ---------------------------------------------------------------------------
if hits=$(grep -rn 'cfg(target_os' crates/core/src 2>/dev/null | grep -vE ':[[:space:]]*(//|/\*)'); then
  fail "no #[cfg(target_os)] in core; platform specifics belong in adapters"
  printf '%s\n' "$hits" >&2
else
  pass "core has no platform conditionals"
fi

# ---------------------------------------------------------------------------
# 3. No absolute developer paths anywhere in Rust.
#
# Home, cache, and config directories come from the `directories` crate.
# ---------------------------------------------------------------------------
if hits=$(grep -rnE '"/(Users|home)/' crates/ --include='*.rs' 2>/dev/null); then
  fail "hardcoded developer path; use the directories crate"
  printf '%s\n' "$hits" >&2
else
  pass "no hardcoded developer paths"
fi

# ---------------------------------------------------------------------------
# 4. packages/transport is the only module that knows about Tauri.
#
# If a component imports @tauri-apps directly, the React code is welded to
# Tauri and the escape route in ADR 0005 becomes fiction.
# ---------------------------------------------------------------------------
tauri_hits=$(grep -rln '@tauri-apps' apps packages --include='*.ts' --include='*.tsx' 2>/dev/null |
  grep -v '^packages/transport/' || true)

if [[ -n "$tauri_hits" ]]; then
  fail "@tauri-apps imported outside packages/transport"
  printf '%s\n' "$tauri_hits" >&2
else
  pass "transport is the only Tauri-aware module"
fi

if ((failed)); then
  printf '\n\033[31mArchitecture invariants violated.\033[0m See docs/architecture.md.\n' >&2
  exit 1
fi

printf '\nAll architecture invariants hold.\n'
