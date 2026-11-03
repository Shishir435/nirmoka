# 0033 — Native macOS is the only desktop shell

Status: Accepted. Supersedes ADR 0003 and the dual-shell transition in ADR 0032.
The TypeScript generation decision in ADR 0010 is retired with its consumer.

## Decision

The product has one desktop interface: `apps/macos`, a SwiftUI application requiring
macOS 13 or later. Remove the React/Tauri shell, JavaScript workspace, generated
TypeScript transport, web-only tests, and Tauri dependencies. The separate static
`site/` remains independent. The Rust CLI, framework-independent core, adapter
contract, fixtures, version gates, and deletion safeguards remain.

Swift starts `nirmoka-native-service` as a private child process. JSON lines on
stdin/stdout carry requests and events; no listening socket is needed. Boundary
DTOs remain in Rust and Swift decoding is checked against the real service in tests.
The tree remains in Rust and only bounded row windows reach Swift.

## User experience

Storage overview is home. Applications, cleanup, Activity, and maintenance tools
have explicit navigation outside Settings. Selecting an installed application can
measure its footprint without replacing the current scan. An inventory footprint
has scan/node ids of zero and is read-only: it never authorizes selected-path
removal. Only backend inventory identifiers are accepted for app removal reviews.

A rescan retains the previous Rust snapshot until a new scan completes. The UI
labels old results and disables scan-derived removal during scanning. Scan ids
remain mandatory, and history/selection reset when the replacement is accepted.
A cancelled or failed scan keeps the previous result available.

Trash, uninstall, and cleanup retain distinct recovery language. Cleanup previews
are snapshots; execution discovers candidates again. Reviewed bytes are never
reported as actual space freed without backend evidence. No new permanent-delete
surface is introduced.

## Build and verification

`apps/macos/run-local.sh` runs locally; `apps/macos/build-app.sh` assembles an
ad-hoc signed app. `build-universal.sh` combines Intel and Apple silicon binaries
for release. CI and Homebrew build Swift and Rust without Node or a webview.
Historical ADRs and completed roadmap steps remain as records, not instructions
for rebuilding the retired shell.
