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

# The uninstall command surface, which is the evidence behind ADR 0027.
#
# Two facts are recorded, both re-checkable after a Mole upgrade: the flags `mo
# uninstall` accepts, and what a named uninstall does when nothing can answer
# its prompt. ADR 0021 read the second as a closed door and withdrew the
# operation. ADR 0027 records what it actually is — a prompt one line of input
# away from a full plan — and `uninstall-plan.txt` below is that plan.
surface="$out/uninstall-command-surface.txt"
# Read the list in full before parsing. An `exit` inside awk would SIGPIPE the
# backend, and `pipefail` would end the recording on a successful read.
inventory=$(mo uninstall --list 2>/dev/null || true)
# Split on the key itself rather than on quotes by position. Mole lists `name`
# before `uninstall_name`, and the two differ for anything installed through
# Homebrew — "LocalSend" is displayed, "localsend" is what the command takes.
# Probing with a display name would record a different code path than the one
# ADR 0021 rests on, so prefer an application where the two disagree: that
# probe proves the identifier matters as well as that the prompt blocks.
app=$(printf '%s\n' "$inventory" | awk '
  /"uninstall_name": "/ {
    split($0, after_name, /"name": "/)
    split(after_name[2], name, "\"")
    split($0, after_id, /"uninstall_name": "/)
    split(after_id[2], id, "\"")
    if (any == "") any = id[1]
    if (name[1] != id[1] && distinct == "") distinct = id[1]
  }
  END { print (distinct != "" ? distinct : any) }')

# The application inventory, sanitized. Shape is the point: this fixture is
# parsed by the adapter, and a hand-written one that disagrees with the backend
# is worse than none — `size` is a rounded *string* here ("410.9MB"), and a
# fixture claiming a byte count let a schema mismatch reach a release.
#
# One entry from each source Mole distinguishes, so the fixture covers both the
# App case and the Homebrew case where the display name and the uninstall
# identifier disagree. `!seen++` rather than an `exit`: the same SIGPIPE that
# `pipefail` turns into a failed recording.
app_row=$(printf '%s\n' "$inventory" | awk '/"source": "App"/ && !seen++')
cask_row=$(printf '%s\n' "$inventory" | awk '/"source": "Homebrew"/ && !seen++')

# A machine with no Homebrew cask, or no App-source application, can only
# produce half a fixture. Writing it anyway would leave a file that looks
# recorded, parses as one entry or not at all, and fails the adapter test that
# reads both rows — with the recording machine, not the fixture, as the cause.
missing=()
[[ -n "$app_row" ]] || missing+=("App")
[[ -n "$cask_row" ]] || missing+=("Homebrew")
if ((${#missing[@]} > 0)); then
  echo "no application with source: ${missing[*]}" >&2
  echo "the inventory on this machine cannot produce the fixture, which needs" >&2
  echo "one row of each source; $out/applications.json is unchanged" >&2
  exit 1
fi

# Every field the adapter reads is rewritten, so the recording machine's own
# applications do not end up in a committed fixture. `"name"` does not match
# inside `"uninstall_name"`: the pattern requires the quote before it.
sanitize() {
  local row=$1 name=$2 uninstall_name=$3
  printf '%s\n' "$row" |
    sed -E \
      -e "s/\"name\": \"[^\"]*\"/\"name\": \"$name\"/" \
      -e "s/\"uninstall_name\": \"[^\"]*\"/\"uninstall_name\": \"$uninstall_name\"/" \
      -e 's/"bundle_id": "[^"]*"/"bundle_id": "com.example.desktop"/' \
      -e 's|"path": "[^"]*"|"path": "/Applications/Example.app"|' \
      -e 's/,$//' \
      -e 's/^[[:space:]]*/  /'
}

# Built beside the fixture and moved into place, so a failure anywhere above
# leaves the previous recording intact rather than a half-written one.
applications="$out/applications.json"
staged="$work/applications.json"
{
  echo "["
  sanitize "$app_row" "Example" "Example" | sed -E 's/$/,/'
  sanitize "$cask_row" "Example Cask" "example-cask"
  echo "]"
} >"$staged"

# The adapter deserializes this file. A fixture that is not JSON is a test
# failure with the wrong error message attached.
if ! node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$staged"; then
  echo "the recorded inventory is not valid JSON; $applications is unchanged" >&2
  exit 1
fi

mv "$staged" "$applications"
echo "recorded $applications ($(wc -c <"$applications" | tr -d ' ') bytes)"

{
  echo "# Recorded from Mole $version by scripts/record-mole-fixture.sh"
  echo "#"
  echo "# Evidence for ADR 0027. Nirmoka parses none of this. It records two things"
  echo "# the uninstall flow depends on: that Mole still prompts, and that it still"
  echo "# routes to the Trash unless --permanent is passed, which Nirmoka never does."
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
    echo "(exit status: $status. With stdin closed the prompt is unanswerable and the"
    echo "plan never prints — which is what ADR 0021 measured. See uninstall-plan.txt"
    echo "for the same command with \`y\` on stdin.)"
  else
    echo "(no installed application was available to probe)"
  fi
} >"$surface"
echo "recorded $surface ($(wc -c <"$surface" | tr -d ' ') bytes)"

# The dry-run plan, which the adapter parses. This is the fixture behind
# ADR 0027: proof that `y` on stdin reaches Mole's complete leftover plan, and
# that `--dry-run` modifies nothing while doing so.
#
# Recorded with the same confirmation the adapter writes, because a fixture
# recorded a different way would test a code path the app never takes.
plan="$out/uninstall-plan.txt"
if [[ -n "$app" ]]; then
  # The probed app's own identifiers, so the recording machine's applications do
  # not reach a committed fixture. Read from the inventory rather than guessed:
  # the bundle id appears inside container and preference paths.
  app_display=$(printf '%s\n' "$inventory" | awk -v id="$app" '
    index($0, "\"uninstall_name\": \"" id "\"") {
      split($0, after, /"name": "/); split(after[2], name, "\""); print name[1]; exit
    }')
  app_bundle=$(printf '%s\n' "$inventory" | awk -v id="$app" '
    index($0, "\"uninstall_name\": \"" id "\"") {
      split($0, after, /"bundle_id": "/); split(after[2], value, "\""); print value[1]; exit
    }')

  set +e
  transcript=$(printf 'y\n' | mo uninstall --dry-run "$app" 2>&1)
  status=$?
  set -e

  {
    echo "# Recorded from Mole $version by scripts/record-mole-fixture.sh"
    echo "#"
    echo "# The plan \`mo uninstall --dry-run <name>\` prints once its prompt is"
    echo "# answered. Evidence for ADR 0027, and the input the adapter's parser is"
    echo "# tested against. Recorded with the same \`y\` the adapter writes."
    echo "#"
    echo "# Exit status was $status. Nothing was modified: --dry-run is set during"
    echo "# flag parsing and every removal below it is separately gated on it."
    echo
    printf '%s\n' "$transcript" |
      sed -e $'s/\033\\[[0-9;]*[a-zA-Z]//g' \
        -e "s|$HOME|~|g" \
        ${app_bundle:+-e "s|$app_bundle|com.example.desktop|g"} \
        ${app_display:+-e "s|$app_display|Example|g"} \
        -e "s|$app|example-cask|g" |
      sed -E -e 's/Last: [^|]*$/Last: <when>/'
  } >"$plan"
  echo "recorded $plan ($(wc -c <"$plan" | tr -d ' ') bytes)"
else
  echo "no installed application was available to probe; $plan is unchanged" >&2
fi

echo
echo "Recorded from Mole $version into $out."
echo "If root.json now contains entries below its immediate children, ADR 0012"
echo "is out of date and the adapter should be reconsidered."
echo "If uninstall-plan.txt no longer shows a complete leftover plan, or shows a"
echo "third prompt after the plan, ADR 0027 is out of date and the uninstall"
echo "confirmation flow must be re-derived before the version gate is widened."
