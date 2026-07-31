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
# 1. Only the shell may depend on a GUI framework.
#
# This is the boundary that makes the frontend replaceable. crates/cli exists
# to turn a violation into a build failure; this catches it a step earlier, and
# catches it for every crate rather than the two that existed when it was
# written — an adapter reaching for `tauri::async_runtime` would compile fine
# and quietly weld a backend to the shell.
# ---------------------------------------------------------------------------
tauri_crates=$(grep -lE '^[[:space:]]*tauri(-build)?[[:space:]]*=' crates/*/Cargo.toml 2>/dev/null |
  grep -v '^crates/app/Cargo.toml$' || true)

if [[ -n "$tauri_crates" ]]; then
  fail "only crates/app may depend on tauri (see docs/adr/0005)"
  printf '%s\n' "$tauri_crates" >&2
else
  pass "tauri is confined to crates/app"
fi

# ---------------------------------------------------------------------------
# 1b. core's dependency list is an allowlist, not a preference.
#
# Invariant 1 is "the standard library, serde, and thiserror". `tauri` is the
# dependency everyone remembers not to add; the one that actually shows up is
# something convenient like serde_json or a JSON parser for a backend format,
# which quietly makes the domain model know about a wire format.
# ---------------------------------------------------------------------------
core_deps=$(awk '
  /^\[dependencies\]/       { in_deps = 1; next }
  /^\[/                     { in_deps = 0 }
  in_deps && /^[a-zA-Z0-9_-]+/ { sub(/[[:space:]].*/, "", $0); print }
' crates/core/Cargo.toml)

unexpected=$(printf '%s\n' "$core_deps" | grep -vE '^(serde|thiserror)?$' || true)

if [[ -n "$unexpected" ]]; then
  fail "crates/core may only depend on serde and thiserror (see AGENTS.md, invariant 1)"
  printf '%s\n' "$unexpected" >&2
else
  pass "core depends only on serde and thiserror"
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
tauri_hits=$(grep -rln '@tauri-apps' apps packages \
  --include='*.ts' --include='*.tsx' \
  --exclude-dir=node_modules --exclude-dir=dist 2>/dev/null |
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
