#!/usr/bin/env bash
#
# Record ncdu export fixtures for the contract suite.
#
# The contract suite must run on machines with no backend installed — Windows
# CI has no ncdu at all — so the awkward cases are recorded once, here, from a
# real backend and committed. Recorded output is never hand-edited afterwards,
# with exactly one exception: the scan root is an absolute path on whichever
# machine did the recording, and that gets rewritten to a stable placeholder.
# Everything else, including the producer version and timestamp, is left as ncdu
# wrote it.
#
# Re-run this after an ncdu upgrade. Fixtures are per version on purpose: a
# format drift should show up as a new directory next to the old one, not as a
# diff that quietly replaces the evidence.
#
# Usage: ./scripts/record-ncdu-fixture.sh

set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v ncdu >/dev/null 2>&1; then
  echo "ncdu is not installed; nothing to record" >&2
  exit 1
fi

version=$(ncdu --version | awk '{print $2}')
case "$version" in
  2.*) ;;
  *)
    echo "refusing to record from ncdu $version; fixtures must come from a supported 2.x" >&2
    exit 1
    ;;
esac

out="fixtures/ncdu/$version"
mkdir -p "$out"

work=$(mktemp -d)
cleanup() {
  # The unreadable directory has to be made readable again before rm can
  # descend into it.
  chmod -R u+rwX "$work" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

tree="$work/tree"

# One tree with every case the parser has to survive.
mkdir -p "$tree/nested/deeper" "$tree/empty" "$tree/unreadable"
printf 'hello world' >"$tree/nested/regular.txt"
head -c 8192 /dev/zero >"$tree/nested/deeper/blocks.bin"

# Hardlink: two names, one inode. Counting it twice would report space that
# deleting one name does not free.
ln "$tree/nested/regular.txt" "$tree/nested/deeper/hardlink.txt"

# Symlink: not a regular file, and its target must not be followed.
ln -s ../regular.txt "$tree/nested/deeper/link"

# Sparse: 4 MB apparent, no blocks allocated.
dd if=/dev/zero of="$tree/sparse.img" bs=1 count=0 seek=4194304 2>/dev/null

# Read error: a directory the scanner cannot descend into. Recorded as a real
# permission failure rather than a synthesised flag.
touch "$tree/unreadable/secret"
chmod 000 "$tree/unreadable"

# ncdu reports the canonical root, and on macOS mktemp hands back a path that
# canonicalises to something else entirely (/var/… → /private/var/…). Rewriting
# the un-canonicalised path would leave half the recording machine's layout in
# the fixture.
tree_real=$(cd "$tree" && pwd -P)

record() {
  local name=$1
  shift
  # The placeholder keeps the recording machine's paths out of the repository
  # and makes path-reconstruction assertions stable.
  ncdu -o - "$@" "$tree" | sed "s|$tree_real|/fixtures/$name|g" >"$out/$name.json"
  echo "recorded $out/$name.json ($(wc -c <"$out/$name.json" | tr -d ' ') bytes)"
}

record simple
record excluded --exclude 'blocks.bin' --exclude 'empty'
record extended -e

# A scan whose root has nothing in it: the degenerate tree, and the one most
# likely to be mishandled by a parser that assumes at least one child.
empty="$work/empty-root"
mkdir -p "$empty"
empty_real=$(cd "$empty" && pwd -P)
ncdu -o - "$empty" | sed "s|$empty_real|/fixtures/empty-root|g" >"$out/empty-root.json"
echo "recorded $out/empty-root.json"

cat >"fixtures/README.md" <<'DOC'
# Fixtures

Recorded output from real backends, committed so the contract suite runs
everywhere — including on CI machines where the backend cannot be installed.

- `ncdu/<version>/` — produced by `scripts/record-ncdu-fixture.sh`.

## Rules

- **Recorded, not written.** Every file here came out of a real backend. Making
  one up defeats the purpose: the suite exists to catch the difference between
  what a backend documents and what it emits.
- **One directory per backend version.** A format drift should appear as a new
  directory beside the old one, so the previous evidence survives.
- **One normalisation, and only one.** The scan root is rewritten to
  `/fixtures/<name>` so the recording machine's home directory does not end up
  in the repository and so path assertions are stable. Nothing else is touched.
- `unreadable/` is recorded from a real `chmod 000` directory, so the
  `read_error` flag in these files is genuine.
DOC

echo "recorded fixtures README"
