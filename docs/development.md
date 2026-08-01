# Development Setup

## Prerequisites

| Tool | Version | Notes                                                           |
| ---- | ------- | --------------------------------------------------------------- |
| Node | ≥ 22    |                                                                 |
| pnpm | 10.32.1 | pinned via `packageManager` in `package.json`                   |
| Rust | stable  | via `rustup`; `rust-toolchain.toml` selects the channel         |
| ncdu | 2.x     | the baseline scanner; `brew install ncdu`                       |
| rip  | 0.13.x  | optional undo for existing receipts; `brew install rm-improved` |

On Linux the shell links against the system webview, which is a package rather than part of
the OS:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libssl-dev patchelf
```

Optional: `mo` ([Mole](https://github.com/tw93/Mole)) for the macOS-only rich backend from
step 9. rip is required only to undo existing receipts; new selected-path deletion is disabled.

## First-time setup

```bash
# Rust, if not already present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# The backend Nirmoka drives
brew install ncdu
brew install rm-improved    # optional: undo existing receipts

git clone git@github.com:Shishir435/nirmoka.git
cd nirmoka

pnpm install
pnpm hooks:install          # enable the pre-push hook (once per clone)
rustup target add x86_64-pc-windows-gnu # catch Windows-only warnings locally
cargo check --workspace --all-targets
```

Verify the whole thing works:

```bash
cargo test --workspace
pnpm typecheck && pnpm build
pnpm nrmk backends          # should report ncdu with your installed version
pnpm nrmk scan .            # a real scan of this repository
```

## Daily commands

```bash
pnpm dev                    # frontend dev server, :5173 (strict port)
pnpm nrmk backends          # real backend detection, no GUI needed
pnpm nrmk backends --json

pnpm nrmk scan ~/Downloads               # largest first
pnpm nrmk scan . --depth 2 --limit 5     # nest two levels, five entries each
pnpm nrmk scan . --json                  # the same window, machine-readable
pnpm nrmk scan / -x --exclude-caches     # one filesystem, skip CACHEDIR.TAG trees

pnpm tauri dev              # the desktop shell; starts Vite for you
pnpm tauri build            # a distributable bundle
pnpm types                  # regenerate the TypeScript mirrors of the Rust DTOs

cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p nirmoka-adapter -p nirmoka-adapter-gdu -p nirmoka-adapter-rip --tests \
  --target x86_64-pc-windows-gnu -- -D warnings
cargo fmt --all
pnpm format
```

`pnpm tauri dev` runs `pnpm dev` for you and opens the window against it, so Vite does not
need to be started separately. `pnpm dev` on its own still works and renders the UI against
the mock transport — useful for styling without a backend or a Rust toolchain.

## Before pushing

The pre-push hook runs this for you once `pnpm hooks:install` has been run. On non-Windows
hosts it also cross-checks the platform-sensitive rip tests for Windows; install that target
once with `rustup target add x86_64-pc-windows-gnu`.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p nirmoka-adapter -p nirmoka-adapter-gdu -p nirmoka-adapter-rip --tests \
  --target x86_64-pc-windows-gnu -- -D warnings
cargo test --workspace
pnpm format:check
pnpm typecheck
pnpm invariants
```

Bypass for one push when you genuinely need to:

```bash
NIRMOKA_SKIP_HOOKS=1 git push
```

Pre-push rather than pre-commit on purpose — clippy and the test suite are too slow to run
on every commit, and a local commit that fails checks harms nobody. Pushing is where it
starts costing CI minutes and other people's attention.

If `cargo` is missing the hook warns loudly and skips the Rust steps rather than making the
repo unpushable. CI is still the gate.

The invariant checks live in `scripts/check-invariants.sh`, called by both the hook and CI,
so the two cannot disagree about what the rules are.

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

`scan` works without any backend at all when pointed at a recorded export, which is how the
parser gets debugged on a machine that cannot install ncdu:

```bash
pnpm nrmk scan --from-export fixtures/ncdu/2.8.2/simple.json
```

This is also the enforcement mechanism for invariant 1 — if `crates/core` ever gains a
`tauri` dependency, this binary stops building. See [ADR 0005](adr/0005-frontend-port.md).

## Changing a type that crosses to TypeScript

The boundary types live in `crates/app/src/dto.rs` and their TypeScript mirrors are
generated into `packages/transport/src/generated/bindings.ts`, which is committed.

```bash
pnpm types      # cargo test -p nirmoka-app export_bindings
```

`cargo test --workspace` rewrites the file too, so in practice the failure mode is a dirty
working tree rather than a forgotten step. The pre-push hook and CI both reject a diff, so
a Rust type cannot reach main without its mirror. Prettier ignores the generated directory —
formatting it would make every regeneration a diff.

Do not edit the generated file. See [ADR 0010](adr/0010-boundary-types-in-the-shell.md) for
why these types are separate from the domain types they mirror.

## Working on the tree view

The list is virtualized and paged, and both halves matter.

- `apps/desktop/src/hooks/use-directory.ts` holds one directory as a **sparse array** of
  `total` slots. A slot is `undefined` until the chunk covering it has been asked for.
- `apps/desktop/src/components/tree-view.tsx` renders only the rows TanStack Virtual says
  are on screen, and calls `ensure(first, last)` from an effect so the chunks covering the
  visible range get requested.
- A row whose chunk has not landed renders as a placeholder of the right height. The
  scrollbar is sized from `total`, which Rust reports for the whole directory.

Two things that look like they belong in the component and do not:

- **Sorting.** It is a parameter on `rows`, because the component holds a window. Sorting
  the rows it happens to have would order the visible slice and leave the rest of the
  directory alone — a screen that looks correctly sorted with the largest file missing from
  it. See [ADR 0011](adr/0011-ordering-and-paging-are-server-side.md).
- **The way back out.** `RowPage.ancestors` carries the chain from the root, so the
  breadcrumb and the up button are rendered from the page rather than from a client-side
  stack that a rescan would invalidate.

`pnpm dev` alone runs against the mock transport, whose fixture tree includes a directory of
500 entries, one that cannot be read, and one that is genuinely empty — the three states the
list has to render differently. No backend or Rust toolchain needed.

## Adding a shadcn/ui component

`apps/desktop` is already configured (`components.json`, Tailwind v4 CSS variables,
`@/lib/utils` with `cn`). No `init` needed:

```bash
cd apps/desktop
pnpm dlx shadcn@latest add button
```

Components land in `src/components/ui/`.

## Fixtures and the contract suite

`tests/contract` runs one suite against every adapter, driven by real backend output
recorded under `fixtures/`. It needs no backend installed, which is what lets it run on
Windows CI.

```bash
cargo test -p nirmoka-contract-tests
./scripts/record-ncdu-fixture.sh     # re-record after upgrading ncdu
```

The recording script builds a small tree containing the cases that break parsers — a
hardlink, a sparse file, a symlink, an unreadable directory, an empty directory — runs the
real backend over it, and rewrites only the scan root so the recording machine's paths stay
out of the repository.

Fixtures live under `fixtures/<backend>/<version>/`. After a backend upgrade, record into
the new version directory and keep the old one: a format drift should be visible as two
directories side by side.

## Scanning a whole home directory

The parser holds the tree in Rust and hands the frontend only what it asks for, so a large
scan is expected to work rather than to be avoided:

```bash
/usr/bin/time -l ./target/release/nrmk scan "$HOME" --limit 10
```

On the development machine that is 2.2M entries, 399 MB peak RSS, and about 50 seconds —
almost all of it ncdu walking the disk. If that number grows sharply after a change to
`Node` or `Tree`, the cost is per-node and worth understanding before it reaches the UI.

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
