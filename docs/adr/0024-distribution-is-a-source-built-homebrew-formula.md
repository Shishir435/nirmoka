# ADR 0024: Distribution is a source-built Homebrew formula

- Status: accepted
- Date: 2026-08-04
- Amends [ADR 0023](0023-the-first-release-is-macos-only.md), which assumed a signed bundle

## Context

ADR 0023 committed to one artifact: a signed, notarized universal macOS bundle. That assumed a
Developer ID certificate, which requires a paid Apple Developer Program membership. There isn't one,
and there will not be one for 0.1.0.

Distribution without it is worse than it used to be. A downloaded `.dmg` carries
`com.apple.quarantine`, and Gatekeeper refuses an unsigned app inside it with "Nirmoka is damaged and
can't be opened. You should move it to the Trash" — a dialog whose only affirmative button destroys
the app. The recovery path is System Settings → Privacy & Security → Open Anyway, after a failed
launch, per app.

A Homebrew **cask** does not help. Homebrew removed `--no-quarantine`
([Homebrew/brew#23363](https://github.com/Homebrew/brew/pull/23363)); cask installs always
quarantine their download, so a cask of an unsigned `.dmg` produces exactly the dialog above.

A Homebrew **formula** is different in the way that matters. It compiles on the user's machine, and a
locally built binary is never quarantined — which is also why `pnpm tauri build` has always worked
for anyone with a checkout.

## Decision

The supported install is a formula in a personal tap, building from source:

```
brew install shishir435/tap/nirmoka
```

`packaging/homebrew/nirmoka.rb` is the source of truth for that formula and is copied into the tap
per release. `version` and `sha256` are the only lines that change; the release workflow prints both
into its job summary rather than leaving them to be guessed.

The formula depends on `ncdu`, because a Nirmoka with no backend can only explain that it has no
backend. Mole stays optional and is named in the caveats.

A `.dmg` is still built and still attached to the release, unsigned, with release notes that say what
it is. It is evidence that the pipeline works and a fallback for anyone who wants it, not the
recommended path.

Releasing an unsigned bundle now requires the repository variable `ALLOW_UNSIGNED_RELEASE=true`. A
tag without both signing credentials and that opt-in fails before the build. The point is that an
unsigned artifact can never reach a release by accident — only on purpose, once, visibly.

## Consequences

Installing takes minutes instead of seconds and needs a Rust and Node toolchain, which Homebrew pulls
in as build dependencies. For an app whose audience already runs `brew install ncdu`, that is a
smaller ask than talking someone through a "damaged application" dialog.

The formula fetches from npm and crates.io during the build, which homebrew-core does not accept.
This stays a personal tap. Vendoring both dependency trees to qualify for core is not worth it at
0.1.0.

The signed path is unchanged and still preferred: `.github/workflows/release.yml` signs, notarizes,
and verifies with `codesign` and `spctl` when the six `APPLE_*` secrets exist, and only then creates
the draft. Buying a certificate later removes the opt-in variable and the unsigned notes, and nothing
else. This ADR is superseded at that point, not amended.
