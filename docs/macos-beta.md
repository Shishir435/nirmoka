# Native macOS preview

SwiftUI is the only desktop interface (ADR 0033). Rust core and adapters remain
independent of the native shell. The old React/Tauri app is removed.

## Launch

1. Install Xcode with Swift 6 and the pinned Rust toolchain.
2. Install a scanner: `brew install ncdu`.
3. Optionally install cleanup and application management: `brew install mole`.
4. Confirm detection: `cargo run -p nirmoka-cli -- backends`.
5. Run `apps/macos/run-local.sh`, or build `apps/macos/build-app.sh` and open
   `target/macos/Nirmoka.app`.

The app always uses the real private Rust service. There is no browser fixture
mode. Choose a folder through the native Scan menu; Applications and setup are
available before any scan. A scan only reads. Incomplete measurements are labeled.

## Safety

Selected items can be moved to the platform Trash after a resolved-path review and
one-time confirmation. Permanent selected-path deletion remains unavailable.
Cleanup and uninstall use the backend's own previews and safety rules; cleanup
runs discover candidates again and do not guarantee recovery. Preview counts are
not presented as a per-path receipt of what was removed. Cancelling an operation
cannot restore changes already made.

Run Rust and Swift checks from development.md and use the native window before
shipping. Signing/notarization and universal release builds are in releasing.md.
