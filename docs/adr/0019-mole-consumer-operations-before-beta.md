# ADR 0019: Mole consumer operations gate the macOS beta

- Status: accepted; the uninstall half superseded by
  [ADR 0021](0021-application-uninstall-is-not-an-adapter-api.md)
- Date: 2026-08-01

## Context

The real scan UI can show a non-technical user where disk space went, but it cannot yet help
them act on that information. Developers can fall back to terminal tools; the intended consumer
cannot. A public beta that ends at diagnosis would demonstrate the interface without delivering
the product's cleanup outcome.

Mole 1.48.1 exposes several distinct command shapes:

- `mo status --json` returns a one-shot JSON document. It also supports newline-delimited JSON
  through `--watch`.
- `mo uninstall --list` emits a JSON application array when stdout is not a terminal. Each item
  includes the exact uninstall name accepted by Mole.
- `mo uninstall --dry-run <name>` performs backend-owned leftover discovery, but its preview is
  human-readable and still participates in an interactive confirmation flow.
- `mo clean --dry-run` writes an exact, grouped path list to
  `~/.config/mole/clean-list.txt`. Its internal ledger is NUL-delimited but temporary and is not
  a public interface. The published file is human-readable, not JSON.
- `mo clean --select`, `--categories`, and `--exclude` were removed. A normal clean performs a
  new discovery and cleans the eligible set; it cannot execute an immutable plan supplied by
  Nirmoka.

These facts were verified against the installed Homebrew release, not inferred from Mole's data
tables. Nirmoka must not copy those GPL-3.0 tables or present its own guesses as Mole decisions.

## Decision

The macOS consumer beta is gated on two complete Mole workflows:

1. cleanup preview, confirmation, execution, and exact result reporting;
2. application inventory, uninstall preview, confirmation, execution, and exact result reporting.
   **Superseded by ADR 0021**: the inventory is implemented, and uninstall is not reachable without
   answering Mole's own confirmation prompt, so it is not part of the beta.

System status is the first implementation because its JSON boundary is already suitable and it
proves the capability-specific adapter shape without destructive behavior.

Mole operations get dedicated contracts. They do not widen `Capabilities::delete`, which remains
caller-selected arbitrary-path deletion. Cleanup categories and application uninstall have
different identifiers, recovery behavior, warnings, and result types.

Every plan must originate from Mole output. Version gates, recorded fixtures, cancellation, and
malformed-output tests apply before a capability reaches the UI.

Cleanup execution must say that Mole re-discovers eligible candidates. The preview is evidence of
what Mole found, not an immutable execution plan. Nirmoka must show the time and completeness of
the preview, expire stale confirmation state, and report execution results independently. It must
not claim that every previewed path—and only every previewed path—will be removed.

Application uninstall defaults to Mole's Trash route. Permanent removal is not the beta default.
The adapter passes Mole's `uninstall_name`, never a UI-assembled path or guessed application name.

Terminal-only administration—Mole update/remove, shell completion, Touch ID setup, and whitelist
editing—does not gate the beta. Optimize, purge, and installer cleanup may follow after the core
consumer loop is proven.

## Consequences

The current read-only build is not a public consumer beta. Signing alone cannot make it one.

Status and application inventory can land before destructive work because they have machine-
readable sources. Cleanup preview can use the published Mole file only under a version-pinned,
fixture-tested parser; Nirmoka never reads Mole's private temporary ledger.

If human-readable preview parsing proves too unstable, work pauses at the adapter boundary and
requests a stable upstream JSON interface. Driving Mole's TUI with synthesized keypresses is not
an alternative.

The frontend remains backend-neutral. It renders cleanup and uninstall domain types from
`@nirmoka/transport`; it does not import Mole-specific process logic or assemble commands.
