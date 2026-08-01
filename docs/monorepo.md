# Monorepo Layout

Nirmoka is a two-language repo: Rust for the core and adapters, TypeScript for the
frontend. Cargo and pnpm both run here, and they do not conflict — they govern **disjoint
directory sets** and neither knows the other exists.

```
nirmoka/
├── Cargo.toml              [workspace] members = ["crates/*"]
├── pnpm-workspace.yaml     packages: apps/*, packages/*
├── rust-toolchain.toml
├── package.json            root scripts only, private
│
├── crates/                 ← Rust, and only Rust
│   ├── core/               domain model. no tauri, no adapters, no target_os
│   ├── adapter/            trait, Capabilities, Detection, Registry
│   ├── adapter-ncdu/       baseline backend
│   ├── adapter-mole/       macOS backend (step 9)
│   ├── adapter-gdu/        cross-platform scanner; Windows default
│   ├── adapter-rip/        exact undo for existing receipts; no new deletion
│   ├── cli/                bin `nrmk` — headless harness
│   └── app/                Tauri shell — window, commands, and nothing else
│
├── apps/                   ← TypeScript
│   └── desktop/            React 19 + Tailwind v4 + shadcn/ui
│
├── packages/
│   └── transport/          the IPC boundary
│
├── site/                   static landing page — NOT in the pnpm workspace
└── docs/
```

## Why the language split is at the top level

`crates/` is Rust, `apps/` and `packages/` are TypeScript. Mixed directories get confusing
within a month — you stop being able to tell what `pnpm -r` will touch, and Cargo's
`members = ["crates/*"]` glob stops being a reliable statement of what the workspace is.

## How Tauri bridges the two

Config, not tooling. `crates/app/tauri.conf.json`:

```json
{
  "build": {
    "frontendDist": "../../apps/desktop/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": { "cwd": "../..", "script": "pnpm dev" },
    "beforeBuildCommand": { "cwd": "../..", "script": "pnpm build" }
  }
}
```

The `cwd` matters: the shell lives in `crates/app` rather than the `src-tauri/` the Tauri
templates assume, so the frontend commands have to be run from the repository root where
pnpm's workspace is.

pnpm builds the frontend, Cargo builds the shell, `pnpm tauri dev` runs both. The Vite dev
server uses `strictPort: true` so a port collision fails loudly instead of silently moving
to 5174 and leaving the shell pointed at nothing.

## Types across the language boundary

This is the real multi-language problem. Rust structs need TypeScript equivalents, and
hand-written mirrors drift silently.

`ts-rs` derives on the boundary types in `crates/app/src/dto.rs` and emits one file,
`packages/transport/src/generated/bindings.ts`. Regenerate with:

```bash
pnpm types          # cargo test -p nirmoka-app export_bindings
```

That output is **committed**, so the frontend builds without a Rust toolchain — the `web`
CI job has no Rust installed and proves it on every run. `cargo test` rewrites the file as
a side effect, and CI fails on a diff, so a Rust type cannot move without its mirror moving
with it. Committed generated code plus a diff check is what makes drift impossible rather
than merely discouraged.

One file rather than one per type: a module per type means an import graph between
generated files, which is a second thing to get right for no gain at this size.

The types being generated live in the shell rather than in `core`, because invariant 1 caps
`core` at serde and thiserror — see [ADR 0010](adr/0010-boundary-types-in-the-shell.md).

(`tauri-specta` produces fully typed `invoke` wrappers and is nicer. It is also more
machinery. Start with `ts-rs`; upgrade if the command surface grows.)

## Why `site/` is outside the pnpm workspace

The landing page is dependency-free static HTML deployed straight to Vercel via
`vercel.json`. Pulling it into the workspace would put a zero-dependency static site into
the app build graph for no benefit, and would mean touching a live deployment to gain
nothing. It stays at the root, deployed independently.

## Tooling deliberately absent

- **Turborepo** — two apps and one package do not need a build graph. Add it the day builds
  actually feel slow.
- **`packages/ui`** — components live in `apps/desktop` until a second consumer exists.
- **A shared `packages/tsconfig`** — two tsconfigs is not duplication worth abstracting.

Every one of these is a five-minute change later, and a permanent tax now.

## Adding a crate

1. `crates/<name>/Cargo.toml` — inherit `version`, `edition`, `license`, `repository`,
   `authors` from `[workspace.package]`.
2. Add it to `[workspace.dependencies]` in the root `Cargo.toml` if others will use it.
3. Use `{ workspace = true }` for every shared dependency so versions stay aligned.
4. If it is an adapter, register it in `crates/cli/src/main.rs` **and** the Tauri app — the
   contract suite checks both registries agree.

## Adding a TypeScript package

1. `packages/<name>/package.json`, name it `@nirmoka/<name>`, mark it `private`.
2. Add a `typecheck` script — the root `pnpm typecheck` runs `-r --if-present`.
3. Consumers depend on it with `"workspace:*"`.
4. If it needs to talk to the backend, it goes **through** `@nirmoka/transport`, never
   around it. CI greps for `@tauri-apps` imports outside that package.

## Commands

```bash
pnpm install                                  # JS deps
pnpm dev                                      # frontend on :5173
pnpm build                                    # typecheck + vite build
pnpm typecheck                                # every JS package
pnpm format                                   # prettier

cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

pnpm nrmk backends                            # cargo run -p nirmoka-cli -- backends
```
