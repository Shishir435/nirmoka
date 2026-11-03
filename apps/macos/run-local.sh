#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cargo build --manifest-path "$repo_root/Cargo.toml" -p nirmoka-app --bin nirmoka-native-service
NIRMOKA_NATIVE_SERVICE="$repo_root/target/debug/nirmoka-native-service" \
  swift run --package-path "$repo_root/apps/macos" NirmokaMac
