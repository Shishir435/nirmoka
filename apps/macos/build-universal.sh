#!/bin/sh
set -eu
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo_root"
# Assemble resources and Info.plist using the same local build path.
apps/macos/build-app.sh
for architecture in arm64 x86_64; do
  case "$architecture" in arm64) rust_target=aarch64-apple-darwin ;; x86_64) rust_target=x86_64-apple-darwin ;; esac
  cargo build --release -p nirmoka-app --bin nirmoka-native-service --target "$rust_target"
  swift build --package-path apps/macos --scratch-path "target/swift-$architecture" --configuration release --arch "$architecture"
done
app_bundle=target/macos/Nirmoka.app
lipo -create target/aarch64-apple-darwin/release/nirmoka-native-service target/x86_64-apple-darwin/release/nirmoka-native-service -output "$app_bundle/Contents/Helpers/nirmoka-native-service"
arm_bin=$(swift build --package-path apps/macos --scratch-path target/swift-arm64 --configuration release --arch arm64 --show-bin-path)
intel_bin=$(swift build --package-path apps/macos --scratch-path target/swift-x86_64 --configuration release --arch x86_64 --show-bin-path)
lipo -create "$arm_bin/NirmokaMac" "$intel_bin/NirmokaMac" -output "$app_bundle/Contents/MacOS/Nirmoka"
codesign --force --sign - --timestamp=none "$app_bundle/Contents/Helpers/nirmoka-native-service"
codesign --force --sign - --timestamp=none "$app_bundle"
for architecture in arm64 x86_64; do
  lipo -verify_arch "$architecture" "$app_bundle/Contents/MacOS/Nirmoka"
  lipo -verify_arch "$architecture" "$app_bundle/Contents/Helpers/nirmoka-native-service"
done
