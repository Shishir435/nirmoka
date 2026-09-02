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

### Rehearsing a release

Three ways to exercise this without claiming a version, in increasing fidelity:

| How                 | What it covers                          | What it produces           |
| ------------------- | --------------------------------------- | -------------------------- |
| `workflow_dispatch` | build, signing, verification            | nothing                    |
| `v0.1.0-rc.1`       | all of the above, plus the release step | a draft marked "rehearsal" |
| `v0.1.0`            | the real thing                          | a draft to publish         |

A tag with a semver prerelease suffix builds the version it rehearses — `v0.1.0-rc.1` checks against
a bundle version of `0.1.0`, not `0.1.0-rc.1` — and the **What the tap needs** step is skipped,
because the tap must never point at a rehearsal. Delete the rc draft and the rc tag afterwards.

A failed run is the pipeline working. Nothing is created until every check ahead of it passed, so a
red run leaves no draft to clean up — only the tag, which `git push --delete origin <tag>` removes.

### Installing from main

The formula carries a `head` line, so a build of current `main` needs no second formula:

```bash
brew install --HEAD nirmoka/tap/nirmoka
```

That, or `pnpm tauri dev` from a checkout, is what "test the dev version" means here. Both install
the same bundle identifier as the release, so they replace it rather than sitting beside it.

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
`homebrew-<tap>`: **`nirmoka/homebrew-tap`**, holding `Formula/nirmoka.rb`. That file is a copy of
[`packaging/homebrew/nirmoka.rb`](../packaging/homebrew/nirmoka.rb), which is the source of truth and
the only one to edit.

Two lines change per release — `url` and `sha256`. **The workflow writes them for you.** Its
**Update the tap** step downloads the source tarball GitHub generated for the tag, hashes it, rewrites
those two lines in a copy of the source-of-truth formula, and pushes the result to the tap. A
hand-copied `sha256` is the most breakable step in this whole pipeline: get one character wrong and
every `brew install` fails checksum verification, for everyone, until someone notices.

That step needs **`TAP_TOKEN`** — a repository secret holding a fine-grained personal access token
with `Contents: read and write` on `nirmoka/homebrew-tap`. The default `GITHUB_TOKEN` is scoped to
this repository and cannot push to another one. Without the secret the step still computes and prints
the two lines, warns loudly that the tap was **not** updated, and leaves it to be done by hand — the
old behaviour, kept as the fallback rather than as the default.

The step is skipped for rehearsal tags, because the tap must never point at one.

Note the ordering: the tap is updated while the release is still a **draft**, so the tarball it points
at does not exist yet and `brew install` fails to download until the draft is published. The workflow
says so in its run summary. Publish the draft promptly.

Verify a change before pushing it to the tap:

```bash
brew tap-new nirmoka/tap --no-git      # local only, no GitHub involved
cp packaging/homebrew/nirmoka.rb "$(brew --repository)/Library/Taps/nirmoka/homebrew-tap/Formula/"
brew style nirmoka/tap
brew audit --formula --strict nirmoka/tap/nirmoka
brew install --build-from-source nirmoka/tap/nirmoka   # the real test, and slow
brew untap nirmoka/tap
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
6. Publish the draft release. Do this before testing the tap: the workflow has already pointed the
   tap at this tag, and the source tarball is not downloadable until the release is out of draft.
7. Check the tap was updated (the run summary says whether it was, and names what is left if it was
   not). Then install it on a clean machine, or at least a clean prefix, and launch the app. That is
   the path users take, so it is the one that has to be checked.
8. If the release is signed, also download the `.dmg`, open it on a Mac that has never seen the app,
   and check that it launches without a Gatekeeper warning. Notarization can be verified without a
   second machine; first-launch behaviour cannot.
9. Update `CHANGELOG.md`: move `[Unreleased]` down under the new version, and add the comparison
   link at the bottom.

## What a first-time user needs

Nirmoka ships no backend and never will — it detects what is installed and guides otherwise. A user
with none of them gets a window that says so. Release notes should name the minimum:

- **ncdu 2.x** for scanning, on any platform (`brew install ncdu`). The formula depends on it, so a
  Homebrew install already has it.
- **Mole 1.48.x** for cleanup, macOS only. Optional, and it is what the Clean page needs.

## Versions to bump together

```bash
./scripts/bump-version.sh 0.3.0
```

That writes all three, rotates the changelog heading, and runs the invariant check. The list below
is what it touches and why each one has to agree — worth knowing when the script is not what you
reach for.

- `crates/app/tauri.conf.json` — the version inside the bundle, and what the workflow checks.
- `Cargo.toml` — `workspace.package.version`, inherited by every crate.
- `packaging/homebrew/nirmoka.rb` — the `url` tag. `./scripts/check-invariants.sh` fails when this
  disagrees with `tauri.conf.json`, so CI catches the bump you forget. The `sha256` cannot be known
  until the tag exists, so the repository copy carries a placeholder and the workflow fills in the
  real one on its way to the tap.

  The version cannot move out of the formula, and it is worth knowing why rather than rediscovering
  it. A tap is a static Ruby file that `brew` reads after cloning it — there is no CI anywhere in a
  user's install path — and the `url` plus `sha256` pin one exact tarball, which is the whole job of
  a formula. Homebrew parses the version out of the url tag, which is why there is no `version`
  stanza: adding one is redundant, and interpolating `v#{version}` into the url fails Homebrew's own
  `FormulaAudit/ComponentsOrder` rule, which requires `url` to come first. So the version stays
  written in the url, and the bump script is what keeps writing it by hand out of the process.

- `CHANGELOG.md` — the `[Unreleased]` heading becomes the version, with a dated entry.

`package.json` files stay at `0.0.0`: they are private workspace members, never published to npm,
and a version there would be a number nobody reads and everybody has to remember to change.
