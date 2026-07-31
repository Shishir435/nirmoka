# ADR 0001 — Drive existing CLI tools through an adapter layer

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

Nirmoka needs disk scanning, size aggregation, and safe deletion. Three options were
available:

1. Write a scanner from scratch.
2. Build directly on one existing tool.
3. Build on existing tools behind an abstraction.

Writing a scanner from scratch means reimplementing hardlink deduplication, sparse-file
handling, permission errors, filesystem-boundary detection, cancellation, and per-platform
metadata quirks — before showing a single pixel. Worse, the _valuable_ part of a tool like
Mole is not its traversal loop, it is years of curated knowledge about which paths are safe
to delete on macOS. That is not reproducible in a side project, and attempting it would
make the app less safe, not more.

Building directly on one tool solves that, but creates a hard dependency on a single
upstream project. Mole is macOS-only, which alone rules out using it as the sole backend
for a cross-platform app.

## Decision

Nirmoka drives existing disk tools as **subprocesses**, behind an adapter trait. Backends
are swappable. No backend code is linked or copied.

The initial backend set is Mole (macOS, rich), ncdu (everywhere, baseline), and gdu
(everywhere, fast, the realistic Windows path).

## Consequences

**Good**

- Cross-platform becomes achievable, since different platforms can use different backends.
- The app inherits each backend's safety rules instead of inventing weaker ones.
- Upstream breakage or abandonment is a contained change.
- Development effort concentrates on the interface, which is the actual gap in the market.
- The process boundary keeps Nirmoka's own license independent of its backends'. See
  [ADR 0004](0004-license-apache-2.md).

**Bad**

- The feature set is bounded by the least capable backend, unless gated behind capability
  flags. This adds real complexity to the UI.
- Subprocess management — streaming, cancellation, orphan prevention — is genuine work.
- Output format drift is an ongoing maintenance cost. Mitigated by version pinning and
  recorded fixtures.
- Users must install a backend themselves. This is a real onboarding cost, accepted
  deliberately in preference to redistributing GPL binaries.

**Neutral**

- Because the boundary is a process boundary, the GUI framework is also replaceable
  without touching product logic. Useful insurance.

## Rejected alternatives

**Write our own scanner.** Rejected: high cost, worse safety, no differentiation. The gap
in the market is interface quality, not scan speed.

**Build only on Mole.** Rejected: macOS-only, and it makes the project a de facto Mole
frontend rather than an independent product.

**Link the backends as libraries.** Rejected: Mole is GPL-3.0, so linking would force
Nirmoka to be GPL-3.0. It would also lose the crash isolation and clean cancellation that
a separate process gives.
