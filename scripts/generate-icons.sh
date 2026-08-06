#!/usr/bin/env bash
#
# Regenerate the bundled icon set from assets/nirmoka-mark.svg.
#
# The mark is committed as SVG because that is the editable form, and the
# platform icons are committed as PNG/ICNS/ICO because a bundler cannot read an
# SVG. Both are checked in for the same reason the ts-rs bindings are: a build
# should not need this script to have been run.
#
# `tauri icon` takes a raster source, so the SVG is rasterized first. There is no
# rsvg-convert or ImageMagick on a stock Mac, and adding one as a build
# dependency to draw one file would be a poor trade — so this uses headless
# Chrome, which is already on any machine that has a browser. Run it after
# editing the mark, and commit what it writes.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# The icon, not the mark. They differ on purpose: the mark is full-bleed for the
# sidebar, where 10% of transparent margin would only make it small, and the icon
# sits on Apple's grid because the Dock puts it beside other icons.
source_svg="$root/assets/nirmoka-icon.svg"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if [[ ! -f $source_svg ]]; then
  echo "missing $source_svg" >&2
  exit 1
fi

browser=""
for candidate in \
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser" \
  "/Applications/Chromium.app/Contents/MacOS/Chromium" \
  "$(command -v chromium || true)" \
  "$(command -v google-chrome || true)"; do
  if [[ -n $candidate && -x $candidate ]]; then
    browser="$candidate"
    break
  fi
done

if [[ -z $browser ]]; then
  echo "no Chromium-based browser found to rasterize the mark" >&2
  echo "install one, or produce a 1024x1024 PNG yourself and run:" >&2
  echo "  pnpm tauri icon <that-file>.png" >&2
  exit 1
fi

# A page the exact size of the icon, with the mark inlined and nothing else — no
# margin, no background, so the rasterized square is the artwork alone. Inlined
# rather than an <img src>, because a referenced file can lose the race with the
# screenshot and produce a blank white square that looks like a working icon.
# `background:none` on the body as well as the flag: the transparent margin is
# the whole point of the icon grid, and a white one would square the corners off
# exactly the way filling the canvas did.
{
  printf '<body style="margin:0;width:1024px;height:1024px;background:none">'
  cat "$source_svg"
  printf '</body>'
} >"$work/icon.html"

"$browser" --headless --disable-gpu --hide-scrollbars \
  --virtual-time-budget=4000 --default-background-color=00000000 \
  --screenshot="$work/icon.png" --window-size=1024,1024 \
  "$work/icon.html" >/dev/null 2>&1

if [[ ! -s $work/icon.png ]]; then
  echo "rasterizing the mark produced nothing" >&2
  exit 1
fi

# A blank render is the failure this script is most likely to have, and it looks
# like success until the app is launched. A PNG of one flat colour compresses far
# smaller than the mark does, so size is enough to catch it.
if [[ $(wc -c <"$work/icon.png") -lt 4000 ]]; then
  echo "the rasterized mark is suspiciously small — probably a blank page" >&2
  exit 1
fi

cd "$root/crates/app"
pnpm exec tauri icon "$work/icon.png"

# Mobile is explicitly out of scope — see docs/roadmap.md. `tauri icon` writes
# iOS and Android sets anyway, and committing them would claim a target that
# does not exist.
rm -rf "$root/crates/app/icons/ios" "$root/crates/app/icons/android"

echo
echo "icons regenerated in crates/app/icons — review and commit them"
