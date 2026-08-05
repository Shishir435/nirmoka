# ADR 0018: selected-path deletion is deferred for v0.1

- Status: partly superseded by
  [ADR 0025](0025-move-to-trash-is-a-platform-integration.md) — recoverable removal ships as a
  platform integration; permanent selected-path deletion remains deferred under ADR 0017's gate
- Date: 2026-08-01

## Context

Step 10 originally required explicit confirmation, recoverable removal, and exact undo for a
path selected from a scan. ncdu and gdu expose deletion only inside interactive terminal UIs,
and Mole removes curated categories rather than arbitrary paths. rip appeared to provide the
missing operation, but ADR 0017 withdrew it because its pathname-only execution cannot be bound
to the object Nirmoka validated.

Keeping Step 10 open would make the first release depend on an unknown future backend. Marking
the capability available would be worse: the product would claim a safety property its backend
cannot provide.

## Decision

- New selected-path deletion is not part of v0.1.
- Every current adapter continues to report `delete: false` and `trash: false`.
- The confirmation-token boundary, deletion validator, and durable operation journal remain in
  place for a future conforming backend; their existence does not imply a usable capability.
- rip continues to report only `undo: true`, preserving exact recovery for receipts created by
  earlier builds.
- The UI hides new-deletion controls. Unsupported capability is product truth, not an error or
  an incomplete screen.
- Step 10 is complete under this reduced, fail-safe scope. It may be reopened only when a
  backend binds execution to the validated filesystem object or provides an equivalent atomic
  containment guarantee without Nirmoka reimplementing deletion.

## Consequences

v0.1 is a read-only disk analyzer for arbitrary paths, with backend-specific abilities exposed
only where their adapters can honor them. Users cannot remove a selected tree row from Nirmoka.

Shipping and UI work no longer wait on speculative destructive infrastructure. Adding selected-
path deletion later requires a new ADR, adapter contract coverage, cancellation tests, durable
receipts, explicit confirmation, and capability-gated UI.
