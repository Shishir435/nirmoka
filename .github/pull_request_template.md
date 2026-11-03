<!-- What changed, and why. The why is the part that is expensive to recover later. -->

## Checks

<!-- `git config core.hooksPath .githooks` runs all of these on push. -->

- [ ] `cargo fmt --all` · `cargo clippy --workspace --all-targets -- -D warnings` · `cargo clippy --workspace -- -D clippy::unwrap_used` · `cargo test --workspace`
- [ ] `swift test --package-path apps/macos` · `apps/macos/build-app.sh` · checked the native window.
- [ ] `./scripts/check-invariants.sh`
- [ ] Touched boundary DTOs? Ran Swift contract tests against the built Rust helper.
- [ ] Added a dependency? Ran `cargo deny check`, and said in the description why the dependency is needed.
- [ ] Touched deletion, trash, or uninstall? Tests came first, and no backend safety rule was reimplemented.
- [ ] A decision worth keeping? Added an ADR in `docs/adr/`.
