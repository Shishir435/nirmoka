# ADR 0006 — One repo, two package managers, split by language at the top level

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

Nirmoka needs Rust (core, adapters, Tauri shell) and TypeScript (React frontend, landing
page). The options were separate repos, or one repo running both Cargo and pnpm.

Separate repos would mean the adapter trait and its TypeScript mirror types living apart,
versioned independently, with a release dance every time a field changes — for a solo
project at step 0. Rejected immediately.

That leaves the real question: how do two package managers coexist without either one's
globs surprising you?

## Decision

**One repo. Language split at the top level.**

```
crates/     Cargo workspace, members = ["crates/*"]
apps/       pnpm workspace
packages/   pnpm workspace
site/       neither — static, deployed independently
```

Cargo and pnpm govern disjoint directory sets. Neither is aware of the other. Tauri bridges
them through `tauri.conf.json`, which invokes pnpm as `beforeDevCommand` /
`beforeBuildCommand` — configuration, not a build-tool integration.

## Rationale

**Split by language, not by feature.** A `features/scan/` directory holding both Rust and
TypeScript would be conceptually tidy and practically miserable: `pnpm -r` and
`members = [...]` stop being readable statements about what the workspace contains, and
within a month nobody can tell what a command will touch.

**Types cross the boundary by generation, not by hand.** Hand-written TypeScript mirrors of
Rust structs drift silently, which is the actual failure this layout has to prevent. From
step 7, `ts-rs` emits `.d.ts` into `packages/transport/src/generated/`. That output is
**committed** so the frontend builds without a Rust toolchain, and CI regenerates it and
fails on a diff. Committed generated code plus a diff check makes drift impossible instead
of merely discouraged.

**`site/` stays out of the pnpm workspace.** It is dependency-free static HTML deployed to
Vercel via `vercel.json`. Adding it to the workspace would insert a zero-dependency static
page into the app build graph and require touching a live deployment for no gain.

## Rejected alternatives

**Separate repos.** Cross-repo coordination for a solo project at step 0. No.

**Turborepo or Nx.** Two apps and one package do not need a build graph. Plain pnpm scripts
plus `cargo` are enough, and Turbo is a five-minute addition the day builds actually feel
slow. Adding it now is a permanent tax for a problem that does not exist yet.

**Rust crates inside `packages/`.** Would make `packages/*` mean two different things and
break the property that a directory tells you which toolchain owns it.

**Cargo as the only workspace, with the frontend as a subdirectory.** Tauri's default
scaffold shape (`src-tauri/` next to a frontend). Works for single-app projects; fights a
monorepo, because the frontend stops being a workspace member with its own dependency graph.

## Consequences

**Good**

- One command per toolchain, and it is obvious which applies where.
- Atomic commits across the language boundary — a Rust type change and its TypeScript
  consumer land together.
- CI can run the two sides as independent jobs and still gate on both.

**Bad**

- Two lockfiles, two dependency-update flows, two ways to be out of date.
- Contributors need both toolchains installed to work on the full stack. Mitigated by
  committing generated types, so frontend-only work needs no Rust.
- `cargo` at the root and `pnpm` at the root can be confused for one system by newcomers.
  `docs/monorepo.md` exists for that.

**Neutral**

- Adding a third language later (a Go adapter, say) fits the same pattern: a new top-level
  directory owned by one toolchain.
