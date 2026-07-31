# Development Setup

## Prerequisites

| Tool | Version | Notes                                                   |
| ---- | ------- | ------------------------------------------------------- |
| Node | ≥ 22    |                                                         |
| pnpm | 10.32.1 | pinned via `packageManager` in `package.json`           |
| Rust | stable  | via `rustup`; `rust-toolchain.toml` selects the channel |
| ncdu | 2.x     | the baseline backend; `brew install ncdu`               |

Optional: `mo` ([Mole](https://github.com/tw93/Mole)) for the macOS-only rich backend from
step 9.

## First-time setup

```bash
# Rust, if not already present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# The backend Nirmoka drives
brew install ncdu

git clone git@github.com:Shishir435/nirmoka.git
cd nirmoka

pnpm install
cargo check --workspace --all-targets
```

Verify the whole thing works:

```bash
cargo test --workspace
pnpm typecheck && pnpm build
pnpm nrmk backends          # should report ncdu with your installed version
```

## Daily commands

```bash
pnpm dev                    # frontend dev server, :5173 (strict port)
pnpm nrmk backends          # real backend detection, no GUI needed
pnpm nrmk backends --json

cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
pnpm format
```

From step 7, `pnpm tauri dev` launches the desktop app and starts Vite automatically.

## Before committing

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm format
pnpm typecheck
pnpm build
```

CI runs all of the above plus the architecture invariant greps, on macOS, Linux, and
Windows. Getting these green locally is the whole pre-flight.

Never pipe a test or check run into `head` or `tail` — the pipeline reports the pager's exit
code, so a failing run reads as green. Let it print in full, or capture to a file and check
the status separately.

## Working without a GUI

`nrmk` drives `nirmoka-core` with no Tauri anywhere, which makes it the fastest way to work
on adapters and the wire format:

```bash
pnpm nrmk backends --json | jq
```

It exits non-zero when no usable backend is found, so scripts and CI can distinguish "no
backend installed" from "detection succeeded".

This is also the enforcement mechanism for invariant 1 — if `crates/core` ever gains a
`tauri` dependency, this binary stops building. See [ADR 0005](adr/0005-frontend-port.md).

## Adding a shadcn/ui component

`apps/desktop` is already configured (`components.json`, Tailwind v4 CSS variables,
`@/lib/utils` with `cn`). No `init` needed:

```bash
cd apps/desktop
pnpm dlx shadcn@latest add button
```

Components land in `src/components/ui/`.

## Troubleshooting

**`Cannot find module 'node:url'` in `vite.config.ts`** — `@types/node` missing from
`apps/desktop`. It is in `devDependencies`; re-run `pnpm install`.

**Vite fails to start instead of picking another port** — intentional. `strictPort: true` is
set because Tauri points at a fixed `devUrl`; a silent port change would leave the shell
loading nothing. Free port 5173.

**`nrmk backends` reports `missing`** — `ncdu` is not on `PATH`. `brew install ncdu`.

**`nrmk backends` reports `unsupported`** — you have ncdu 1.x or 3.x. Only 2.x is supported;
the JSON export format differs. This is deliberate: an untested version is never treated as
compatible.

**Clippy complains about paths in deleted worktrees** — `cargo clean` and retry.
