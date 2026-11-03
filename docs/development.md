# Development

The desktop app is native SwiftUI. macOS 13+, Xcode 16+ (Swift 6), and the pinned
Rust toolchain are required. There is no Node, pnpm, React, or Tauri app build.

```sh
apps/macos/run-local.sh
apps/macos/build-app.sh
# Faster local bundle for inspection:
NIRMOKA_BUILD_CONFIGURATION=debug apps/macos/build-app.sh
```

The native app launches the Rust helper from its bundle. `run-local.sh` sets
`NIRMOKA_NATIVE_SERVICE` to the debug helper when running through Swift Package
Manager. Backends are installed separately: `brew install ncdu`, and optionally
`brew install mole`. Never bundle a backend binary or copy Mole's data tables.

## Checks

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace -- -D clippy::unwrap_used
cargo test --workspace
cargo build -p nirmoka-app --bin nirmoka-native-service
NIRMOKA_NATIVE_SERVICE="$PWD/target/debug/nirmoka-native-service" swift test --package-path apps/macos
./scripts/check-invariants.sh
cargo deny check
```

After UI changes, build the app and inspect the real window. Tests must precede
changes to deletion behavior. Rust holds all trees and confirmation tokens; Swift
receives bounded rows and cannot execute a removal by sending a raw path.

Enable the pre-push checks with `git config core.hooksPath .githooks`.
