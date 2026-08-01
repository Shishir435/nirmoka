# Architecture

Nirmoka is a desktop frontend with swappable backends. It writes no disk scanner of its
own. Every scan, every size number, and every deletion is performed by an existing,
proven command-line tool running as a separate process.

This document explains the layers and the rules that keep them separate.

## Layers

```
┌───────────────────────────────────────────────┐
│  ui/               React + TypeScript          │
│  Rendering, navigation, treemap, keybindings.  │
│  Knows nothing about any backend.              │
└───────────────────────┬───────────────────────┘
                        │  Tauri commands + events
┌───────────────────────┴───────────────────────┐
│  app/              Tauri glue                  │
│  Command handlers, event streaming, window.    │
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
    pub uninstall_apps: bool,    // application removal with leftovers
    pub system_status: bool,     // health metrics
}
```

**4. Adapters own path validation. The UI never builds a delete command.**

A path travels from the UI to the adapter as data and is validated at the adapter
boundary before it ever reaches a subprocess argument. The GUI layer is the least
trustworthy place in the system to make a deletion decision, because it is the layer
closest to user input and the furthest from the backend's safety rules.

Selected-path deletion uses a two-call boundary. `prepare_delete` keeps the validated
adapter plan in Rust and returns only a one-time confirmation token; `confirm_delete`
consumes that token. A raw path is never accepted by the execute command. No current backend
offers this capability: rip's later pathname resolution cannot be bound to validation, so it
fails closed and retains only exact undo for existing receipts.

**5. Adapters version-pin their backend and fail closed.**

Each adapter declares which backend versions it has been tested against, probes the
installed version at startup, and refuses to run against an unknown one. Output formats
drift silently, and a silently-changed field on a _delete_ path is the worst possible
place to discover that.

**6. `crates/core` must not depend on Tauri either.**

The adapter trait is only half the shape. `core` sits between two ports and depends on
neither side, which is what makes the _frontend_ replaceable as well as the backend.
`crates/cli` (`nrmk`) links `core` with no Tauri anywhere, so a violation is a build
failure rather than a review miss. See [ADR 0005](adr/0005-frontend-port.md).

**7. `packages/transport` is the only module that may import `@tauri-apps/*`.**

Components import from `@nirmoka/transport`. If a component calls `invoke()` directly, the
React code is welded to Tauri and the escape route becomes fiction. Leaving Tauri should
rewrite one file, not the app. CI greps for violations.

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

See [ADR 0003](adr/0003-tech-stack-tauri.md).
