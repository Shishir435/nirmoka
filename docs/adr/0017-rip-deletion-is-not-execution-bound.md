# ADR 0017: rip deletion is not execution-bound

- Status: accepted
- Date: 2026-08-01
- Supersedes: [ADR 0016](0016-rip-is-the-selected-path-deletion-backend.md)

## Context

ADR 0016 accepted rip 0.13.1 for recoverable selected-path deletion. The adapter
canonicalised and checked a target during preparation and again immediately before spawning
rip. That closes ordinary stale-confirmation and symlink-retarget cases, but not the final
time-of-check/time-of-use gap.

rip accepts a pathname. It canonicalises that pathname and later moves it. Another process
can replace an ancestor after Nirmoka's containment check but before rip resolves or moves the
path, redirecting execution outside the confirmed scan root. Nirmoka cannot bind a filesystem
handle into rip's pathname-only interface, and moving the item itself would reimplement the
backend operation forbidden by the deletion safety rules.

The application journal also converted append or synchronization failures into `logError`
while returning deletion success. That could leave no application-readable receipt after a
restart.

## Decision

- Withdraw rip's `delete` and `trash` capabilities. Both `prepare_delete` and `delete` return
  `Unsupported`, including for a manually constructed plan.
- Keep rip's `undo` capability so durable receipts created by an earlier build remain usable.
  `undo` therefore does not imply that a backend may create new recoverable deletions.
- Treat a failed deletion-journal append as failure. No operation is added to in-memory state
  until its JSON Lines event has been written and synchronized.
- Keep the shared deletion validator and confirmation boundary. A future backend may reuse
  them only if its execution can be bound to the object that passed validation.

## Consequences

No current backend offers arbitrary selected-path deletion. The UI and resolver hide the
capability instead of claiming safety the backend interface cannot provide. Existing rip
receipts can still be restored exactly with `--unbury`.

Step 10 is reopened for confirmation and Trash. A future implementation needs an API that
accepts an already-bound filesystem object, performs its own atomic containment guarantee, or
otherwise closes the validation-to-execution race without bypassing backend safety rules.

[ADR 0018](0018-selected-path-deletion-is-deferred-for-v0-1.md) subsequently closes Step 10
by deferring new selected-path deletion beyond v0.1. The safety requirement above remains the
acceptance gate for reopening it.

The operation journal can no longer report a safely recorded deletion after its durable write
failed. This is defense in depth while new deletion is unavailable and a requirement for any
future backend.
