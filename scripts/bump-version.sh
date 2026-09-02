#!/usr/bin/env bash
#
# Bump the release version everywhere it is written down.
#
# Three files carry the version and they must agree: a bundle that says one
# thing while the formula says another produces a release whose name is not what
# it contains, and the mismatch is only visible to whoever installs it.
# scripts/check-invariants.sh fails on that drift, but catching a manual edit is
# worse than not needing one.
#
# The version cannot leave the formula. A tap is a static Ruby file that `brew`
# reads after cloning it — there is no CI in a user's install path — and the url
# plus sha256 pin one exact tarball, which is the formula's entire job. Homebrew
# parses the version out of the url tag, so there is no `version` stanza to set
# instead; adding one is redundant and, if interpolated, fails Homebrew's own
# ComponentsOrder rule, which requires `url` before `version`.
#
# The sha256 is not touched here. It is the hash of the tarball GitHub generates
# for a tag, so it cannot be known until the tag exists. The release workflow
# computes it and writes it on the way to the tap — see docs/releasing.md.
#
# Usage:
#   ./scripts/bump-version.sh 0.3.0

set -euo pipefail

cd "$(dirname "$0")/.."

version=${1:-}

if [[ -z "$version" ]]; then
  echo "usage: $0 <version>   e.g. $0 0.3.0" >&2
  exit 1
fi

# No leading v: the tag carries one, the versions do not.
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "not a version: '${version}' — expected x.y.z, with no leading v" >&2
  exit 1
fi

current=$(grep -m1 '^version = ' Cargo.toml | sed 's/version = "\(.*\)"/\1/')

if [[ "$current" == "$version" ]]; then
  echo "already at ${version}; nothing to do"
  exit 0
fi

echo "${current} -> ${version}"

# `[workspace.package]`, inherited by every crate. Anchored to the first
# `version = ` at column zero so that a dependency's version is never rewritten.
perl -0pi -e "s/^version = \"\Q${current}\E\"/version = \"${version}\"/m" Cargo.toml

# The version inside the bundle, and the one the release workflow checks the tag
# against.
perl -0pi -e "s/(\"version\": \")\Q${current}\E(\")/\${1}${version}\${2}/" crates/app/tauri.conf.json

# The formula's url tag. The sha256 beside it stays a placeholder on purpose.
perl -0pi -e "s{(archive/refs/tags/v)\Q${current}\E(\.tar\.gz)}{\${1}${version}\${2}}" \
  packaging/homebrew/nirmoka.rb

# The changelog's Unreleased section becomes this version, dated, and a fresh
# empty Unreleased opens above it. The comparison links at the foot move with it.
if [[ -f CHANGELOG.md ]]; then
  today=$(date +%Y-%m-%d)
  perl -0pi -e "s{^## \\[Unreleased\\]\n}{## [Unreleased]\n\n## [${version}] — ${today}\n}m" CHANGELOG.md
  # The `[unreleased]` link compares from the last *released* version, which is
  # not necessarily the version Cargo.toml currently carries — 0.2.0 was bumped
  # and never tagged, so the link still pointed at v0.1.1. Read whatever it
  # actually compares from rather than assuming it is $current.
  previous=$(sed -n 's|^\[unreleased\]: .*/compare/v\(.*\)\.\.\.HEAD$|\1|p' CHANGELOG.md | head -1)
  if [[ -n "$previous" ]]; then
    perl -0pi -e "s{^\\[unreleased\\]: (.*)/compare/v\Q${previous}\E\.\.\.HEAD\$}{[unreleased]: \${1}/compare/v${version}...HEAD\n[${version}]: \${1}/compare/v${previous}...v${version}}mi" CHANGELOG.md
  else
    echo "note: could not find an [unreleased] compare link; add the ${version} link by hand" >&2
  fi
fi

echo
echo "updated:"
grep -m1 '^version = ' Cargo.toml
grep -m1 '"version"' crates/app/tauri.conf.json
grep -m1 'archive/refs/tags' packaging/homebrew/nirmoka.rb

echo
./scripts/check-invariants.sh

cat <<EOF

Next:
  1. Move this release's entries under the new CHANGELOG heading, if they are
     not already there.
  2. Commit:  git commit -am "chore(release): ${version}"
  3. Tag:     git tag -a v${version} -m "v${version}" && git push origin v${version}

The formula's sha256 stays a placeholder. The release workflow computes the real
one and writes it to the tap — see docs/releasing.md.
EOF
