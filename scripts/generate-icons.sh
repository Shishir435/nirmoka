#!/usr/bin/env bash
# Export a replacement macOS iconset with the standard macOS tools.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $# != 1 || ! -d "$1" || "$1" != *.iconset ]]; then
  echo "usage: $0 /path/to/Nirmoka.iconset" >&2
  exit 1
fi
iconutil --convert icns "$1" --output apps/macos/Resources/Nirmoka.icns
