# ADR 0010 — The types that cross to TypeScript live in the shell

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

Step 7 needed TypeScript mirrors of the Rust types the frontend reads. `ts-rs` generates
them from a `#[derive(TS)]`, which means something in the workspace has to carry that
derive, and the obvious candidates are the types that already exist: `nirmoka_core::Node`,
`nirmoka_adapter::Detection`, `nirmoka_adapter::Capabilities`.

Deriving on those is ruled out twice over.

**Invariant 1.** `crates/core` depends on the standard library, serde, and thiserror. A
`ts-rs` derive is a fourth dependency, added so that a GUI can exist — which is the exact
coupling `crates/cli` was built to make impossible.

**They are not the same types.** A `Row` is a `Node` plus its share of the parent and how
many children it has. Share is a fact about a viewport, not about a file. A `Detection`
carries a `PathBuf`, which has no TypeScript equivalent that survives Windows. Deriving on
the domain types would mean growing them fields the domain does not use, and every one of
those fields would then be visible to the CLI, the contract suite, and every adapter.

## Decision

**A separate set of DTOs in `crates/app/src/dto.rs`, with `From` conversions from the
domain types, is what carries `#[derive(TS)]`.**

They generate into a single committed file, `packages/transport/src/generated/bindings.ts`,
written by `cargo test` and checked by CI for a diff.

The conversion functions are the whole point. When the UI needs a field the domain does not
have, the change is visible as a conversion getting longer, rather than as `core` quietly
growing something only React wanted.

## Consequences

**Good**

- `core` stays at three dependencies, and the CLI still proves it.
- The IPC surface is one file that can be read start to finish.
- Byte counts cross as `number` rather than ts-rs's default `bigint`, which is the right
  call for a JSON transport and would have been wrong to force onto the domain types.
- A second shell — Electron, a web build — reimplements `dto.rs` and reuses everything else.

**Bad**

- Two definitions of shapes that mostly look alike, and a conversion to keep in step. The
  conversions are covered by tests, and a missing field is a compile error rather than a
  silent `undefined`, but the duplication is real and will grow.
- `crates/app` now has a reason to be read by anyone changing a domain type, which is one
  more place to look.

## Notes

`tauri-specta` generates typed `invoke` wrappers and would remove the hand-written
`tauriTransport()` bindings in `packages/transport`. It is also more machinery, and that
file is thirty lines. Revisit when the command surface is large enough that a typo in a
command name is a realistic failure — the current six commands are all exercised by the
shell's own tests.

Errors cross as strings for now. Step 10 gives deletion failure modes the UI must branch
on; that is when this becomes a typed error, in its own ADR.
