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
# 1. The application is native-only (ADR 0033).
# ---------------------------------------------------------------------------
if grep -qE '^name = "(tauri|tauri-build|ts-rs)"$' Cargo.lock; then
  fail "retired desktop dependencies remain in Cargo.lock"
else
  pass "no Tauri or TypeScript generation dependencies"
fi
if [[ -d apps/desktop || -d packages/transport ]]; then
  fail "the native macOS app must be the only desktop shell"
else
  pass "native macOS is the only desktop shell"
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
# 4. Native bundle, Cargo workspace, and Homebrew version agree.
# ---------------------------------------------------------------------------
formula='packaging/homebrew/nirmoka.rb'
bundled=$(sed -n '/<key>CFBundleShortVersionString<\/key>/{n;s/.*<string>\(.*\)<\/string>.*/\1/p;}' apps/macos/Info.plist)
workspace=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
tagged=$(grep -o 'archive/refs/tags/v[^"]*\.tar\.gz' "$formula" | head -1 |
  sed 's|archive/refs/tags/v\(.*\)\.tar\.gz|\1|')

if [[ -z "$bundled" || -z "$tagged" ]]; then
  fail "could not read a version from apps/macos/Info.plist or $formula"
elif [[ "$bundled" != "$tagged" || "$bundled" != "$workspace" ]]; then
  fail "formula is v${tagged} but the bundle is ${bundled}"
  printf '  update the url in %s, or bump apps/macos/Info.plist\n' "$formula" >&2
else
  pass "the formula and the bundle agree on ${bundled}"
fi

# A syntax error in the formula is only discovered by whoever runs `brew
# install` next. `brew style` is not usable here — run against a loose file it
# applies Homebrew's internal Sorbet and frozen-string rules that real formulae
# are exempt from, and it also sees a locally tapped copy as a duplicate. A
# parse check is the part that is actually about this file.
if command -v ruby > /dev/null 2>&1; then
  if ruby -c "$formula" > /dev/null 2>&1; then
    pass "the formula parses"
  else
    fail "the formula is not valid Ruby"
    ruby -c "$formula" >&2 || true
  fi
else
  printf '      ruby not found; skipped the formula parse check\n'
fi

if ((failed)); then
  printf '\n\033[31mArchitecture invariants violated.\033[0m See docs/architecture.md.\n' >&2
  exit 1
fi

printf '\nAll architecture invariants hold.\n'
