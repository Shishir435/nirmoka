#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
output_dir=${NIRMOKA_OUTPUT_DIR:-"$repo_root/target/macos"}
app_bundle="$output_dir/Nirmoka.app"
configuration=${NIRMOKA_BUILD_CONFIGURATION:-release}
case "$configuration" in debug) cargo_profile="" ;; release) cargo_profile="--release" ;; *) echo "Invalid configuration" >&2; exit 1 ;; esac

cargo build --manifest-path "$repo_root/Cargo.toml" $cargo_profile -p nirmoka-app --bin nirmoka-native-service
swift build --package-path "$repo_root/apps/macos" --configuration "$configuration"

rm -rf "$app_bundle"
mkdir -p "$app_bundle/Contents/MacOS" "$app_bundle/Contents/Helpers" "$app_bundle/Contents/Resources"
swift_bin_dir=$(swift build --package-path "$repo_root/apps/macos" --configuration "$configuration" --show-bin-path)
cp "$swift_bin_dir/NirmokaMac" "$app_bundle/Contents/MacOS/Nirmoka"
cp "$repo_root/target/$configuration/nirmoka-native-service" "$app_bundle/Contents/Helpers/nirmoka-native-service"
cp "$repo_root/apps/macos/Info.plist" "$app_bundle/Contents/Info.plist"

cp "$repo_root/apps/macos/Resources/Nirmoka.icns" "$app_bundle/Contents/Resources/Nirmoka.icns"

# Local build artifact. Release signing and notarization stay in the existing
# release workflow, which owns the production identity and entitlements.
codesign --force --sign - --timestamp=none "$app_bundle/Contents/Helpers/nirmoka-native-service"
codesign --force --sign - --timestamp=none "$app_bundle"

printf 'Built %s\n' "$app_bundle"
