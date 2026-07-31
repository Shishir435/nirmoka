# ADR 0007 — `nrmk` is a development harness, not a product

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

[ADR 0005](0005-frontend-port.md) requires a binary that links `nirmoka-core` without
Tauri, so that the framework-independence claim is enforced by the compiler rather than by
reviewer discipline.

That binary needs a name and a stated scope, because "a CLI in the Nirmoka repo" invites two
wrong conclusions: that users are meant to install it, and that it deserves product-quality
UX and documentation.

Naming candidates were `nirmoka` and `nrmk`.

## Decision

**Crate `nirmoka-cli`, binary `nrmk`, `publish = false`, excluded from releases and from
the user-facing README.**

## Rationale

**It is not a product, deliberately.** Nirmoka's premise is "a GUI for existing CLI tools".
Shipping a CLI that wraps a CLI muddies that story for no user benefit — anyone who wants a
terminal disk analyser should use ncdu directly, which is exactly what Nirmoka would be
calling.

**`nrmk`, not `nirmoka`, for two reasons.** It will be typed several hundred times during
development, so four characters is worth real ergonomics. More importantly it leaves
`nirmoka` **unclaimed as a binary name**, so if a user-facing CLI is ever justified, the
obvious name is still free. Consonant-skeleton naming is well precedented: `rg`, `fd`, `gh`.

**It stays forever, even though it ships to nobody.** Four benefits, none of which are
visible while everything is working:

1. `crates/core` gaining a `tauri` dependency becomes a build failure.
2. CI exercises the whole stack with no display server.
3. Adapters and the wire format can be debugged without launching a window.
4. A broken GUI leaves a working tool behind rather than nothing.

## Consequences

**Good**

- Invariant 1 from `AGENTS.md` is compiler-enforced.
- Fastest possible iteration loop on adapter work.
- The contract test suite (step 6) drives it directly instead of poking a GUI.

**Bad**

- A second binary to keep compiling and passing clippy, forever, for a benefit that only
  shows up when something has gone wrong.
- Its output format will be tempting to treat as an API. It is not; it may change freely.
- Someone will eventually ask to ship it. This ADR is the answer.

## If this is ever revisited

Shipping a user-facing CLI would need: a stable output contract, real argument validation,
help text worth reading, and a positioning story that does not undercut the GUI. That is a
new ADR superseding this one — not a quiet flip of `publish = false`.
