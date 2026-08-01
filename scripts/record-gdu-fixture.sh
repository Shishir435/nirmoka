#!/usr/bin/env bash
#
# Record gdu's ncdu-compatible JSON export for the contract suite.
#
# Usage:
#   ./scripts/record-gdu-fixture.sh
#   GDU=/path/to/gdu ./scripts/record-gdu-fixture.sh

set -euo pipefail

cd "$(dirname "$0")/.."

gdu_bin=${GDU:-gdu}
if ! command -v "$gdu_bin" >/dev/null 2>&1; then
  echo "gdu is not installed; nothing to record" >&2
  exit 1
fi

version=$($gdu_bin --version | awk '$1 == "Version:" {sub(/^v/, "", $2); print $2; exit}')
case "$version" in
  5.32.*) ;;
  *)
    echo "refusing to record from gdu $version; the adapter accepts 5.32.x" >&2
    exit 1
    ;;
esac

out="fixtures/gdu/$version"
mkdir -p "$out"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

tree="$work/tree"
mkdir -p "$tree/nested/deeper" "$tree/empty"
printf 'hello world' >"$tree/nested/regular.txt"
head -c 8192 /dev/zero >"$tree/nested/deeper/blocks.bin"
ln "$tree/nested/regular.txt" "$tree/nested/deeper/hardlink.txt"
ln -s ../regular.txt "$tree/nested/deeper/link"
dd if=/dev/zero of="$tree/sparse.img" bs=1 count=0 seek=4194304 2>/dev/null

tree_real=$(cd "$tree" && pwd -P)
"$gdu_bin" --config-file /dev/null --no-progress -o - "$tree" \
  | sed -e "s|$tree_real|/fixtures/gdu-simple|g" -e "s|$tree|/fixtures/gdu-simple|g" \
  >"$out/simple.json"

empty="$work/empty-root"
mkdir -p "$empty"
empty_real=$(cd "$empty" && pwd -P)
"$gdu_bin" --config-file /dev/null --no-progress -o - "$empty" \
  | sed -e "s|$empty_real|/fixtures/gdu-empty-root|g" \
    -e "s|$empty|/fixtures/gdu-empty-root|g" \
  >"$out/empty-root.json"

echo "recorded $out/simple.json"
echo "recorded $out/empty-root.json"
