# ADR 0020: A cleanup run is journalled without a receipt

- Status: accepted
- Date: 2026-08-03

## Context

Step 10 established one durable, append-only JSON Lines journal for destructive operations, and
one rule about writing to it: a recoverable deletion is not successful until its undo receipt is
durable, so a failed append must not produce an in-memory success.

Mole cleanup execution does not fit that shape. `mo clean` accepts no paths and no categories, it
re-discovers eligible candidates when it runs (ADR 0019), and it reports scope, progress, and
warnings rather than a per-path result. There is nothing to undo and nothing to restore from. The
question is therefore what a journal entry for a cleanup run may claim, and what a failed write
to it means.

Two wrong answers are available and both look reasonable:

1. Journal the reviewed preview rows as the removed set. It reads like a receipt, it is what a
   user wants to see, and it is a fabrication: Mole discovered its own set, minutes later, under
   its own protection rules.
2. Apply the deletion rule unchanged and fail the operation when the append fails. The removal
   has already happened inside the backend and cannot be undone, so this would discard the only
   record of it in the name of durability.

## Decision

Cleanup runs are recorded in the same journal, in the same id space, as a distinct `cleaned`
event. One file, one writer, one sequence, so "operation 4" names one event.

A cleanup entry records exactly two kinds of fact, separately labelled:

- **What was reviewed** — the backend and its exact version, when the preview was generated, the
  reviewed item count, and the reviewed size. This is what the user approved.
- **What the backend reported** — the system scope it ran with, whether it finished or was
  partial, and its own warning lines.

It records no per-path result, because Mole publishes none. `reviewed_items` is never renamed to
something that reads as "removed".

A failed journal write on a cleanup run reports the failure beside the result rather than
replacing it. The result is returned with a `log_error`, and the run stays in the in-memory list.
This inverts the deletion rule deliberately: there, durability _is_ the operation's safety
property; here, the operation is already irreversible and hiding it loses the only record.

The reviewed preview is dropped after a run, successful or not. A refused, cancelled, or failed
run cannot prove that nothing was removed, so its preview is a statement about the past and a
second run must review a fresh discovery. The confirmation token is spent either way.

## Consequences

The UI can show a cleanup history across launches, and it must describe those numbers as
reviewed rather than removed — the frontend copy is part of this decision, not decoration on it.

Nirmoka cannot answer "which files did that cleanup delete". That is a real product limitation
and the honest one: only Mole knows, and it does not say. If Mole later publishes an exact
executed-path result, this ADR is superseded rather than amended, because the entry would then
carry a third kind of fact that the current wording forbids.

A cleanup run cannot be undone through Nirmoka. Mole's own recovery behavior is Mole's; the
journal is a record, not a rollback log.
