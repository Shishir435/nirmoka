<!-- What changed, and why. The why is the part that is expensive to recover later. -->

## Checks

<!-- `pnpm hooks:install` runs all of these on push. -->

- [ ] `cargo fmt --all` · `cargo clippy --workspace --all-targets -- -D warnings` · `pnpm rs:lint:strict` · `cargo test --workspace`
- [ ] `pnpm lint` · `pnpm format:check` · `pnpm typecheck` · `pnpm build`
- [ ] `./scripts/check-invariants.sh`
- [ ] Touched `dto.rs`? Ran `pnpm types` and committed the regenerated bindings.
- [ ] Added a dependency? Ran `pnpm rs:deny`, and said in the description why the dependency is needed.
- [ ] Touched deletion, trash, or uninstall? Tests came first, and no backend safety rule was reimplemented.
- [ ] A decision worth keeping? Added an ADR in `docs/adr/`.
