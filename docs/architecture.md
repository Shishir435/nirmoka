# Architecture

Nirmoka is a desktop frontend with swappable backends. It writes no disk scanner of its
own. Every scan, every size number, and every deletion is performed by an existing,
proven command-line tool running as a separate process.

This document explains the layers and the rules that keep them separate.

## Layers

```
┌───────────────────────────────────────────────┐
│  apps/macos        SwiftUI          │
│  Rendering, navigation, native dialogs.  │
│  Knows nothing about any backend.              │
└───────────────────────┬───────────────────────┘
                        │  JSON-lines requests + events
┌───────────────────────┴───────────────────────┐
│  crates/app        Rust service                  │
│  Command handlers, event streaming, state.    │
└───────────────────────┬───────────────────────┘
                        │  domain types
┌───────────────────────┴───────────────────────┐
│  core/             Domain + policy             │
│  Tree model, sizes, selection, confirmation    │
│  rules. Compiles without any adapter crate.    │
└───────────────────────┬───────────────────────┘
                        │  Adapter trait
┌───────────────────────┴───────────────────────┐
│  adapter/          Trait, registry, caps       │
└───┬───────────────────┬───────────────────┬──────────────┘
    │                   │                   │              │
┌───┴────────┐  ┌───────┴──────┐  ┌─────────┴───┐  ┌───────┴─────┐
│adapter-mole│  │ adapter-ncdu │  │ adapter-gdu │  │ adapter-rip │
└───┬────────┘  └───────┬──────┘  └─────────┬───┘  └───────┬─────┘
    │ subprocess        │ subprocess        │ subprocess           │ subprocess
┌───┴────────┐  ┌───────┴──────┐  ┌─────────┴───┐  ┌───────┴─────┐
│  mo(1)     │  │   ncdu(1)    │  │   gdu(1)    │  │   rip(1)    │
└────────────┘  └──────────────┘  └─────────────┘  └─────────────┘
```

## Native macOS shell

`apps/macos` is a SwiftUI shell for the macOS beta. It starts
`nirmoka-native-service` from `crates/app` as a child process and exchanges
newline-delimited JSON over stdin and stdout. It opens no local socket. The helper
calls the shared Rust command functions; Swift owns presentation,
navigation, and native dialogs, while the Rust adapters still own backend calls,
version checks, previews, and confirmation tokens.

This is the only desktop shell. The React/Tauri application and its TypeScript
transport are retired by [ADR 0033](adr/0033-native-macos-is-the-only-desktop-shell.md). Mole commands
that expose only interactive terminal pickers are not surfaced as GUI actions until an
exact review and confirmation path exists. See
[ADR 0032](adr/0032-native-macos-shell.md).

## Rules

These are structural, not stylistic. Breaking one collapses the design.

**1. `core/` must not depend on any adapter crate.**

Enforced as a dependency rule in the workspace manifest, not as a convention. If `core`
can name a backend, backend assumptions will leak into it, and the adapters stop being
swappable in practice even though the trait still exists.

**2. The wire format is ncdu's JSON export, not Mole's output.**

Mole is the richest backend, so designing the interface around its output is the tempting
mistake. Do that and the ncdu adapter becomes impossible to write — you will have built a
Mole GUI with a backend-shaped hole in it. Building against the _narrowest_ backend keeps
the abstraction honest. The Mole adapter's job is translating down into the common format
and reporting its extra abilities through capability flags.

See [ADR 0002](adr/0002-wire-format-ncdu-json.md).

**3. Every backend ability is a capability flag.**

Backends differ enormously. ncdu scans but exposes deletion only inside its interactive
terminal UI. Mole cleans by category and uninstalls applications, but does not remove an
arbitrary selected path. The UI queries capabilities and hides what the active backend
cannot do, rather than offering a button that fails at call time.

```rust
pub struct Capabilities {
    pub scan: bool,              // walk a directory tree
    pub delete: bool,            // non-interactive selected-path removal
    pub trash: bool,             // recoverable delete rather than permanent
    pub undo: bool,              // exact non-interactive restore
    pub dry_run: bool,           // preview the exact delete list first
    pub cleanup_categories: bool,// named cleanup targets, not just paths
    pub app_inventory: bool,     // list apps with the backend's own identifier
    pub uninstall_apps: bool,    // non-interactive application removal
    pub system_status: bool,     // health metrics
    pub optimize: bool,          // reviewed system maintenance
}
```

Shell integrations — Reveal in Finder, Quick Look, moving to the Trash — are deliberately
_not_ capability flags. They involve no disk tool, they work with no backend installed,
and every adapter would implement them identically. They live in `crates/app` and report
what the desktop offers as data. See
[ADR 0022](adr/0022-shell-integrations-are-not-adapter-abilities.md).

Trash is the one destructive member of that list, so it keeps the destructive machinery
rather than the capability flag: the shared validator, the one-time confirmation token,
and a journal entry. `delete` and `trash` stay `false` on every adapter, because they
describe what a _backend_ can do and no backend does this. See
[ADR 0025](adr/0025-move-to-trash-is-a-platform-integration.md).

**4. Adapters own path validation. The UI never builds a delete command.**

A path travels from the UI to the adapter as data and is validated at the adapter
boundary before it ever reaches a subprocess argument. The GUI layer is the least
trustworthy place in the system to make a deletion decision, because it is the layer
closest to user input and the furthest from the backend's safety rules.

Selected-path deletion uses a two-call boundary. `prepare_delete` keeps the validated
adapter plan in Rust and returns only a one-time confirmation token; `confirm_delete`
consumes that token. A raw path is never accepted by the execute command. No current backend
offers this capability: rip's later pathname resolution cannot be bound to validation, so it
fails closed and retains only exact undo for existing receipts. New selected-path deletion is
deferred beyond v0.1 by
[ADR 0018](adr/0018-selected-path-deletion-is-deferred-for-v0-1.md).

Moving a selected path to the **Trash** reuses that same two-call boundary and the same
validator while involving no adapter at all, because it is a platform integration rather
than a backend ability. Permanent removal stays deferred; recoverable removal ships. See
[ADR 0025](adr/0025-move-to-trash-is-a-platform-integration.md).

**5. Adapters version-pin their backend and fail closed.**

Each adapter declares which backend versions it has been tested against, probes the
installed version at startup, and refuses to run against an unknown one. Output formats
drift silently, and a silently-changed field on a _delete_ path is the worst possible
place to discover that.

**6. `crates/core` must not depend on any GUI framework.**

The adapter trait is only half the shape. `core` sits between two ports and depends on
neither side, which is what makes the _frontend_ replaceable as well as the backend.
`crates/cli` (`nrmk`) links `core` without a desktop framework, so a violation is a build
failure rather than a review miss. See [ADR 0005](adr/0005-frontend-port.md).

**7. Swift talks to Rust only through the private service boundary.**

`NativeServiceClient` owns process communication. Views call `AppModel`; the Rust
service owns validation, capability/version checks, and confirmation tokens. The
retired Tauri shell and TypeScript generator must not return to the build graph.

**8. No backend binary is bundled.**

Nirmoka detects what is installed and guides the user to install a backend if none is
found. This avoids inheriting redistribution obligations from GPL-licensed backends, and
it is more honest to users about what is actually executing on their machine.

## Contract tests

One test suite in `tests/contract/` runs against **every** adapter, using recorded real
backend output stored in `fixtures/<backend>/<version>/`.

Without this, the adapter pattern is documentation rather than architecture. If only the
Mole path is exercised, the ncdu path breaks silently and is discovered on the day Mole
breaks — which is the exact day the fallback was supposed to save you.

Build the ncdu adapter **second, early**, while the trait is still cheap to change. If it
is built last, Mole's assumptions will already be baked into `core/`.

## Why the GUI layer is also replaceable

The adapter boundary is a process boundary, which means the backend and the frontend can
be swapped independently. If Tauri turns out to be the wrong choice, the product logic in
`core/` survives the move. This is a deliberate side effect of the design, not an
accident.

See [ADR 0033](adr/0033-native-macos-is-the-only-desktop-shell.md).
