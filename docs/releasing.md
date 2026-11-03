# Releasing the native macOS app

The product is a SwiftUI app with a bundled private Rust service. No Tauri,
JavaScript build, or webview is involved. Minimum macOS version: 13.

## Local verification

Run the checks in development.md, build `apps/macos/build-app.sh`, launch
`target/macos/Nirmoka.app`, and use the real window. Check setup, a real scan,
folder navigation, application footprints, and preview-only cleanup/removal.
Do not exercise a real removal on user files as a smoke test.

`apps/macos/build-universal.sh` builds Apple silicon and Intel versions of both
executables and combines them with lipo. Install both Rust darwin targets first:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
apps/macos/build-universal.sh
```

Local bundles are ad-hoc signed. CI signs and notarizes only when credentials
are configured. A successful local build is not evidence of notarization.

## Versions

`./scripts/bump-version.sh 0.3.0` updates workspace Cargo.toml, the native
Info.plist's CFBundleShortVersionString, and the Homebrew source URL. The
invariants script checks agreement. The formula checksum is a placeholder in
this repository; the release workflow calculates the source tarball checksum.
Historical release versions remain in CHANGELOG.md.

## Release workflow

Push an explicitly approved `vX.Y.Z` tag to run `.github/workflows/release.yml`.
A prerelease suffix marks a rehearsal and does not update the Homebrew tap.
The workflow builds a universal app, creates a DMG, verifies signing when
configured, and creates a **draft** release for human review.

Signing requires APPLE_CERTIFICATE (base64 PKCS#12), APPLE_CERTIFICATE_PASSWORD,
APPLE_SIGNING_IDENTITY, APPLE_ID, APPLE_PASSWORD, and APPLE_TEAM_ID. The certificate
is imported into a temporary runner keychain. Both the Rust helper and app are
signed with hardened runtime before notarization and stapling.

Without complete signing credentials, tag builds fail unless the repository
variable ALLOW_UNSIGNED_RELEASE=true explicitly permits unsigned distribution.
A manual workflow run can produce an unsigned test artifact without publishing.
Unsigned downloads are subject to Gatekeeper; the source-built Homebrew formula
remains the supported install route while there is no Developer ID certificate.

TAP_TOKEN, when configured, permits updating the separate nirmoka/homebrew-tap
repository after a non-prerelease tag build. The workflow copies the source
formula from packaging/homebrew and substitutes only the release URL/checksum.
Without that token the workflow reports the required manual update.

## Homebrew

The source formula builds Rust and Swift, installs the app from target/macos,
and provides a nirmoka launcher pointing inside its bundle. It requires Xcode
with Swift 6. ncdu is an install dependency; Mole remains optional. No backend
binary is embedded in the app.
