# Contributing to Nirmoka

The architecture is settled enough to take contributions. `crates/adapter` survived
adding a second backend without changing `crates/core` — which was the test — so the
adapter contract is now something you can build against rather than something still
moving under you.

Start with [`docs/architecture.md`](docs/architecture.md), then
[`docs/adapters.md`](docs/adapters.md) if you are adding a backend.
[`docs/roadmap.md`](docs/roadmap.md) is the tracker and says what is in scope now.

## Before you open a pull request

```bash
pnpm install

# Rust
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
pnpm rs:lint:strict          # no unwrap() in shipping code
cargo test --workspace

# Frontend
pnpm lint && pnpm format:check && pnpm typecheck && pnpm build

# Architecture
./scripts/check-invariants.sh
```

`pnpm hooks:install` runs all of that on `git push`, which is cheaper than finding out
from CI. `NIRMOKA_SKIP_HOOKS=1 git push` skips it when you mean to.

If you touched `crates/app/src/dto.rs`, run `pnpm types` and commit the regenerated
`packages/transport/src/generated/bindings.ts`. CI fails on a diff there.

If you added a dependency, run `pnpm rs:deny` (`cargo install cargo-deny --locked`).

## The five invariants

These are in [`AGENTS.md`](AGENTS.md) and CI enforces every one. They are not style
preferences — breaking one collapses the architecture, and a PR that needs to break one
is a PR that needs a conversation first, not a workaround.

1. `crates/core` depends on nothing but std, serde, and thiserror.
2. `packages/transport` is the only place that may import `@tauri-apps/*`.
3. No `#[cfg(target_os)]` in `crates/core`.
4. The wire format is ncdu's JSON export.
5. The tree lives in Rust; the frontend receives only the visible window.

## Deletion

Deletion is the entire risk surface; everything else is recoverable. Anything touching it
follows three rules without exception:

- **Tests first.** No exceptions, and no "the test comes in the follow-up".
- **Never reimplement a backend's safety rules.** Mole's protected-path logic is stricter
  than anything this project should attempt. Call the backend and let it apply them.
- **Degrade, don't lie.** If a backend cannot do something, report `Unsupported`. Never
  fake a dry-run preview by guessing what the backend would delete.

**Never copy Mole's data tables into this repo.** Mole is GPL-3.0. Transcribing its
protected-path arrays or cleanup lists — even "just as data" — would make Nirmoka a
derivative work and silently relicense the project. `deny.toml` enforces the dependency
half of this; the copy-paste half is on us. See [`NOTICE.md`](NOTICE.md).

## Adding a backend

1. A new crate, `crates/adapter-<name>`, implementing the `Adapter` trait.
2. Declare what it can do with `Capabilities` flags. A backend that cannot scan says so;
   it does not scan badly.
3. Record real output under `fixtures/<name>/<version>/` with a script in `scripts/`.
   Fixtures are recorded, never handwritten — a handwritten fixture tests your idea of the
   format rather than the format.
4. Version-gate it. An untested version is `UnsupportedVersion`, not an optimistic `Found`.
5. Pass `tests/contract/` unchanged. Needing a special case there means the trait is wrong,
   and that is worth knowing.
6. Cancellation must kill the subprocess rather than orphan it, with a test that checks the
   pid is gone — not merely that the call returned.

## Commits and pull requests

Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`), with the
area in parentheses where it helps: `fix(adapter): …`.

Say what changed and why. The why is the part that is expensive to recover later, and this
repository's history is deliberately readable.

Anything that will still matter in six months gets an ADR in `docs/adr/`, numbered
sequentially and never deleted. A reversed decision gets a new ADR that marks the old one
superseded, because the reasoning behind a decision you later dropped is still evidence.

## Platform

Development is macOS-first because that is the machine available, but the code stays
platform-neutral. `crates/core` has no platform conditionals, paths are `PathBuf`, and
home and cache directories come from the `directories` crate — never from a `~/Library` or
`/Users/` literal. Linux and Windows are compiled and tested in CI; only macOS is packaged,
for the reason in [ADR 0023](docs/adr/0023-the-first-release-is-macos-only.md).
