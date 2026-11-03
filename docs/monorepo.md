# Repository layout

- `apps/macos`: the only desktop shell, built with Swift Package Manager.
- `crates/app`: Rust application state, command functions, DTOs, and the private
  JSON-lines native service. No GUI framework dependency.
- `crates/core`: framework-independent domain/tree logic.
- `crates/adapter*`: existing CLI backends, capability declarations, validation,
  cancellation, and version gates.
- `crates/cli`: headless harness for the shared Rust layers.
- `tests/contract`, `fixtures`: shared backend contract and recorded evidence.
- `packaging/homebrew`: source-built macOS install.
- `site`: independent, zero-dependency static landing page.

See ADR 0033 for retirement of the React/Tauri shell. Historical ADRs keep their
original paths and rationale; the current entry points are in development.md.
