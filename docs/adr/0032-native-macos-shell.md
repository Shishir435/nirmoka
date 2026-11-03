# ADR 0032: A native SwiftUI shell for macOS

- Status: accepted
- Date: 2026-10-20

## Context

Nirmoka's desktop window is currently Tauri with React. The macOS beta needs a native
interface that fits the platform and can expose the storage, Mole cleanup, software,
maintenance, and system-status workflows in one place. Replacing the Rust core or
calling Mole directly from Swift would duplicate safety rules and make the GUI a second
backend implementation.

Mole's CLI is the supported process boundary. Its native Mole App is a separate product,
and its proprietary implementation is not available for reuse. Mole is GPL-3.0; Nirmoka
keeps its existing license and must not copy Mole code or its cleanup/protection tables.

## Decision

Add `apps/macos` as a SwiftUI shell. It launches `nirmoka-native-service`, a child
process in `crates/app`, and sends newline-delimited JSON over stdin/stdout. The helper
uses the same Rust command functions, adapter registry, version gates, preview data, and
one-time confirmation tokens as the Tauri shell. Swift does not spawn Mole or construct
backend arguments.

Keep the Tauri shell until the native app covers the complete product flow. Build the
native interface as a local preview while the workflows are brought across. Do not
present a Mole command as a GUI action when its current CLI only offers an interactive
terminal picker that cannot be reviewed and confirmed through the adapter contract.

## Consequences

- Rust remains the single owner of scanning, the tree, adapter selection, path handling,
  dry-run output, backend version checks, and confirmation boundaries.
- The two shells share behavior through Rust functions, not copied business logic.
- Native UI changes need a Swift build and a window-level check in addition to the Rust
  service build.
- Mole's terminal-only Purge and Installer pickers remain unavailable in the native UI
  until they have exact adapter contracts. The proprietary Mole App's extra controls
  are not part of this integration.
- This adds no dependency on Mole-App source and changes no license.
