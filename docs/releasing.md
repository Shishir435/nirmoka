# Releasing

The macOS beta is the only shipped build. Other platforms compile and test in CI and are not
packaged — see [ADR 0023](adr/0023-the-first-release-is-macos-only.md).

**How users install it is the Homebrew formula, not the `.dmg`** — see
[ADR 0024](adr/0024-distribution-is-a-source-built-homebrew-formula.md). A release produces both, and
the formula is the one the release notes lead with.

## What produces a release

`.github/workflows/release.yml`, on a `v*` tag. It builds a universal bundle for Apple silicon and
Intel, signs and notarizes it when the credentials are there, verifies the result, and only then
opens a **draft** release with the `.dmg` attached. Publishing is a person's click, always.

Two things about that order. A tag that would produce an unsigned bundle fails before the build
unless someone opted into exactly that, so an unsigned artifact cannot reach a release by accident.
And the draft is created after `codesign` and `spctl` pass rather than by the build step, so a
notarization failure cannot leave a refused `.dmg` sitting in a draft waiting to be published.

`workflow_dispatch` runs the same build without creating a release. Use it to check the pipeline
without producing an artifact anyone can find.

### Releasing unsigned, on purpose

Set the repository **variable** (not secret) `ALLOW_UNSIGNED_RELEASE=true`. Without it a tag missing
any signing secret fails; with it the build proceeds, the signature checks are skipped, and the
release notes gain a section saying the `.dmg` is unsigned and what that means. This is the current
state of the project: there is no Developer ID certificate, so 0.1.0 releases unsigned and users
install through Homebrew instead.

Remove the variable the day a certificate exists. It is an opt-in for one specific compromise, not a
setting.

## Building one locally

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
pnpm tauri build --target universal-apple-darwin
```

Produces `target/universal-apple-darwin/release/bundle/dmg/Nirmoka_<version>_universal.dmg` and the
`.app` beside it. Recorded from 0.1.0 on macOS 26, so a later build that differs sharply is worth a
second look:

- 5.1 MB `.dmg`, universal — `lipo -archs` reports `x86_64 arm64`
- `LSMinimumSystemVersion` 12.0, `LSApplicationCategoryType` `public.app-category.utilities`
- `codesign -dv` reports an **ad-hoc, linker-signed** binary, and `spctl --assess` rejects it with
  "no usable signature"

That last line is what an unsigned build is: it runs on the machine that built it and is refused
everywhere else. It is the expected result of a local build, and an unacceptable one for a release.

## The Homebrew tap

The published formula lives in a separate repository, because a tap must be named
`homebrew-<tap>`: **`Shishir435/homebrew-tap`**, holding `Formula/nirmoka.rb`. That file is a copy of
[`packaging/homebrew/nirmoka.rb`](../packaging/homebrew/nirmoka.rb), which is the source of truth and
the only one to edit.

Two lines change per release — `url` and `sha256`. The workflow's **What the tap needs** step prints
both into the run summary, already formatted; guessing either produces a formula that installs for
nobody.

Verify a change before pushing it to the tap:

```bash
brew tap-new shishir435/tap --no-git      # local only, no GitHub involved
cp packaging/homebrew/nirmoka.rb "$(brew --repository)/Library/Taps/shishir435/homebrew-tap/Formula/"
brew style shishir435/tap
brew audit --formula --strict shishir435/tap/nirmoka
brew install --build-from-source shishir435/tap/nirmoka   # the real test, and slow
brew untap shishir435/tap
```

The install is the only step that proves the formula works, and it pulls `rust` and `node` as build
dependencies, so it is worth doing once per release rather than once per edit.

## Credentials

Six repository secrets, all required for a **signed** release. A tagged build refuses to start
without them unless `ALLOW_UNSIGNED_RELEASE=true` is set, and a `workflow_dispatch` run without them
produces an **unsigned** bundle that the workflow warns about: an unsigned app on a stranger's Mac
reports itself as damaged, and the workaround is teaching people to strip quarantine attributes,
which is a bad habit to teach for a tool that deletes files. The formula exists so that nobody has to
be taught it.

| Secret                       | What it is                                                      |
| ---------------------------- | --------------------------------------------------------------- |
| `APPLE_CERTIFICATE`          | Developer ID Application certificate, exported as base64 `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | The password set when exporting that `.p12`                     |
| `APPLE_SIGNING_IDENTITY`     | e.g. `Developer ID Application: Your Name (TEAMID)`             |
| `APPLE_ID`                   | The Apple ID that owns the developer account                    |
| `APPLE_PASSWORD`             | An app-specific password, not the account password              |
| `APPLE_TEAM_ID`              | The 10-character team identifier                                |

A Developer ID certificate requires a paid Apple Developer account. There is no way around this for
a distributable macOS app, and a self-signed certificate does not satisfy Gatekeeper.

Export the certificate with:

```bash
# From Keychain Access, export the Developer ID Application certificate as
# certificate.p12, then:
base64 -i certificate.p12 | pbcopy
```

## Cutting a release

1. Decide the version. `crates/app/tauri.conf.json` and the workspace `Cargo.toml` must already
   agree with it; the workflow refuses a tag that disagrees with the bundle.
2. `cargo test --workspace && pnpm lint && pnpm build` locally. CI runs these too, but a failed
   release build wastes a tag.
3. `pnpm tauri dev` and use the window. A release is the one build where "it compiles" is not
   evidence — scan a directory, generate a cleanup preview, and check the backend list.
4. Tag and push:

   ```bash
   git tag -a v0.1.0 -m "v0.1.0"
   git push origin v0.1.0
   ```

5. Wait for the workflow. A red run means no draft exists, which is the intended outcome of any
   failure here. A green run with the signature step **skipped** means the release is unsigned, which
   is expected until there is a certificate.
6. Update the tap: copy `packaging/homebrew/nirmoka.rb` into `Shishir435/homebrew-tap`, with the two
   lines from the run summary. Then install it on a clean machine, or at least a clean prefix, and
   launch the app. That is the path users take, so it is the one that has to be checked.
7. If the release is signed, also download the `.dmg`, open it on a Mac that has never seen the app,
   and check that it launches without a Gatekeeper warning. Notarization can be verified without a
   second machine; first-launch behaviour cannot.
8. Edit the release notes, then publish.

## What a first-time user needs

Nirmoka ships no backend and never will — it detects what is installed and guides otherwise. A user
with none of them gets a window that says so. Release notes should name the minimum:

- **ncdu 2.x** for scanning, on any platform (`brew install ncdu`). The formula depends on it, so a
  Homebrew install already has it.
- **Mole 1.48.x** for cleanup, macOS only. Optional, and it is what the Clean page needs.

## Versions to bump together

- `crates/app/tauri.conf.json` — the version inside the bundle, and what the workflow checks.
- `Cargo.toml` — `workspace.package.version`, inherited by every crate.
- `packaging/homebrew/nirmoka.rb` — the `url` tag and its `sha256`, from the run summary.

`package.json` files stay at `0.0.0`: they are private workspace members, never published to npm,
and a version there would be a number nobody reads and everybody has to remember to change.
