# Native macOS app

The SwiftUI target is a native macOS window. Rust remains responsible for
backend detection, scan execution, the scan tree, classification, and paging.
The app starts `nirmoka-native-service` as a child process and exchanges
newline-delimited JSON over its standard input and output. No local network
socket is opened.

The preview has a Storage overview, folder browser, application footprint inspector,
cleanup and app-removal reviews, maintenance, system status, and Activity. Mole-generated cleanup, uninstall, and optimization plans are
reviewed before their one-time confirmation is consumed. The preview does not yet
surface Mole's terminal-only Purge and Installer pickers or the proprietary Mole App's
extra controls.

## Run locally

From the repository root:

```sh
apps/macos/run-local.sh
```

This requires the Swift 6 command-line tools and the repository's Rust toolchain.
The service path can be overridden with `NIRMOKA_NATIVE_SERVICE` when running
the Swift package directly.

## Build a local app bundle

```sh
apps/macos/build-app.sh
```

The script creates `target/macos/Nirmoka.app` for the current architecture and
ad-hoc signs the app and its helper. Release signing and notarization remain in
the release workflow. This is the only desktop shell; see ADR 0033.
