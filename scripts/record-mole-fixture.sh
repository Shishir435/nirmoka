#!/usr/bin/env bash
#
# Record `mo analyze --json` output for the Mole adapter.
#
# Unlike the ncdu fixtures, these are not scan inputs — Nirmoka never parses
# them. They are *evidence*: the recorded proof that Mole's analyzer returns one
# directory's children rather than a tree, which is why the Mole adapter reports
# `scan: false`. See docs/adr/0012-mole-is-not-a-scanner.md.
#
# Recording both levels of the same tree is the point. `nested` appears in the
# top-level output with its full recursive size and no children; its children
# only exist in a second recording, from a second invocation. That is the shape
# the ADR rests on, and re-running this after a Mole upgrade is how the decision
# gets re-tested rather than assumed.
#
# Usage: ./scripts/record-mole-fixture.sh

set -euo pipefail

cd "$(dirname "$0")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Mole is macOS-only; nothing to record" >&2
  exit 1
fi

if ! command -v mo >/dev/null 2>&1; then
  echo "mo is not installed; nothing to record" >&2
  exit 1
fi

# Mole's --version output opens with a blank line, so the version is on the
# first non-empty one rather than the first.
version=$(mo --version | awk 'NF {print $3; exit}')
case "$version" in
  1.*) ;;
  *)
    echo "refusing to record from Mole $version; the adapter is gated to 1.x" >&2
    exit 1
    ;;
esac

out="fixtures/mole/$version"
mkdir -p "$out"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

tree="$work/tree"
mkdir -p "$tree/nested/deeper"
printf 'hello world' >"$tree/small.txt"
head -c 65536 /dev/zero >"$tree/nested/big.bin"
printf 'x' >"$tree/nested/deeper/leaf.txt"
ln -s small.txt "$tree/link.txt"

# mktemp on macOS hands back /var/…, which canonicalises to /private/var/….
# Mole reports the path it was *given*, uncanonicalised — unlike ncdu, which
# reports the canonical form — so both spellings have to be rewritten or the
# recording machine's layout ends up in the fixture.
tree_real=$(cd "$tree" && pwd -P)

record() {
  local name=$1 target=$2
  mo analyze --json "$target" \
    | sed -e "s|$tree_real|/fixtures/mole|g" -e "s|$tree|/fixtures/mole|g" \
    >"$out/$name.json"
  echo "recorded $out/$name.json ($(wc -c <"$out/$name.json" | tr -d ' ') bytes)"
}

record root "$tree"
record nested "$tree/nested"

# The uninstall command surface, which is the evidence behind ADR 0021.
#
# Two facts are recorded, both re-checkable after a Mole upgrade: the flags `mo
# uninstall` accepts, and what a named uninstall does when nothing can answer
# its prompt. Neither the plan nor the removal is reachable without writing to
# that prompt, which is why the adapter refuses the operation instead of
# synthesizing an answer.
surface="$out/uninstall-command-surface.txt"
# Read the list in full before parsing. An `exit` inside awk would SIGPIPE the
# backend, and `pipefail` would end the recording on a successful read.
inventory=$(mo uninstall --list 2>/dev/null || true)
app=$(printf '%s\n' "$inventory" | awk -F'"' '/uninstall_name/ && !seen++ {print $4}')

{
  echo "# Recorded from Mole $version by scripts/record-mole-fixture.sh"
  echo "#"
  echo "# Evidence for ADR 0021. Nirmoka parses none of this; it is the proof that"
  echo "# uninstall cannot be driven non-interactively."
  echo
  echo "== mo uninstall --help =="
  mo uninstall --help 2>&1
  echo
  echo "== mo uninstall --dry-run <app> with stdin closed =="
  if [[ -n "$app" ]]; then
    set +e
    transcript=$(mo uninstall --dry-run "$app" </dev/null 2>&1)
    status=$?
    set -e
    # Strip ANSI control sequences, the probed app's name, and the matched line,
    # which carries this machine's app size and last-used time.
    printf '%s\n' "$transcript" |
      sed -e $'s/\033\\[[0-9;]*[a-zA-Z]//g' -e "s|$app|Example|g" |
      sed -E 's/^[0-9]+\. .*/1. Example  <size>  |  Last: <when>/'
    echo
    echo "(exit status: $status, and the plan never printed)"
  else
    echo "(no installed application was available to probe)"
  fi
} >"$surface"
echo "recorded $surface ($(wc -c <"$surface" | tr -d ' ') bytes)"

echo
echo "Recorded from Mole $version into $out."
echo "If root.json now contains entries below its immediate children, ADR 0012"
echo "is out of date and the adapter should be reconsidered."
echo "If uninstall-command-surface.txt now shows a non-interactive flag, ADR 0021"
echo "is out of date and uninstall should be reconsidered."
