# ADR 0023: The first release is macOS-only

- Status: accepted
- Date: 2026-08-04

## Context

The roadmap planned step 11 to end with three packaged builds: a signed macOS app, a Linux AppImage
or Flatpak, and CI producing installers for all three platforms. That plan was written before the
Mole work, and the product it describes is not the product that now exists.

What shipped is a cleanup loop: find a storage problem, review a backend-produced plan, approve it,
run it, read the result. Every destructive part of that is Mole, and Mole is macOS-only — its
installer refuses other platforms and its analyze sources are `//go:build darwin`. On Linux and
Windows, Nirmoka today is a directory browser over ncdu or gdu. That is real and useful
infrastructure, and it is not the thing worth asking someone to install a desktop app for.

Packaging has a cost that does not end at the first build. An AppImage or Flatpak is a signing
story, a runtime story, a webview-dependency story, and an ongoing "it broke on distro X" surface —
paid per release, against a build whose headline feature is absent on that platform.

## Decision

Version 0.1.0 ships one artifact: a signed, notarized universal macOS bundle, built by
`.github/workflows/release.yml` on a `v*` tag and published from a draft.

Linux packaging is dropped from step 11 rather than deferred inside it, so the roadmap stops
describing work nobody is doing. Windows packaging was never in step 11 and stays out.

**This changes what is packaged, not what is built.** The five invariants are untouched, `core`
stays platform-neutral, gdu remains the Windows scanner (ADR 0015), and CI keeps compiling and
testing the workspace. The macOS-only CI matrix is a beta-duration measure with the other entries
commented in place, not a decision to stop caring whether the code compiles elsewhere.

Unsigned bundles are not published. The workflow builds one when credentials are absent and says
so, because a dry run needs to be possible — but shipping it would mean telling users to strip
quarantine attributes from an application that deletes files.

## Consequences

Someone on Linux who wants Nirmoka builds it from source. That is a real cost and the honest one:
the alternative is a package promising a Clean page that reports every operation as unsupported.

A second cleanup backend — one that works off macOS — is what changes this decision, not demand for
a package. Until then, Linux packaging would be distributing a browser for tools that already have
excellent terminal interfaces.

The release pipeline is not platform-specific by accident. Adding a target later is a matrix entry
and a bundle configuration, not a rewrite, and this ADR gets superseded rather than amended.
