# ADR 0016: rip is the selected-path deletion backend

- Status: superseded by [ADR 0017](0017-rip-deletion-is-not-execution-bound.md)
- Date: 2026-08-01
- Extends: [ADR 0014](0014-interactive-deletion-is-not-an-adapter-api.md)

## Context

ADR 0014 established that ncdu, gdu, and Mole cannot remove an arbitrary path selected
from Nirmoka's scan tree. ncdu and gdu expose deletion only inside interactive terminal
browsers; Mole cleans curated categories and uninstalls named applications. Calling
`std::fs`, `rm`, or a platform API behind those adapters would bypass the product's
backend safety boundary.

Step 10 still needs a concrete selected-path operation, recoverability, exact undo, and an
audit trail. [rip (rm-improved)](https://github.com/nivekuil/rip) 0.13.1 provides a
non-interactive path argument, moves the item to a graveyard rather than unlinking it,
records the original and recovery paths, and accepts an exact graveyard path through
`--unbury`. It is GPL-3.0 and Unix-only upstream; invoking a separately installed binary
does not link or copy it into Nirmoka.

## Decision

- Add `adapter-rip`, version-gated to `>=0.13, <0.14`, as an optional macOS/Linux backend.
- Declare selected-path delete, Trash, and exact undo. Do not declare scanning, permanent
  deletion, or dry run.
- Give every operation a dedicated graveyard below the application-data directory. This
  avoids name conflicts and makes an undo receipt identify exactly one item.
- Validate the target during preparation and again immediately before spawning rip, so a
  symlink retarget or containment escape is refused.
- Keep the prepared plan in Rust. The transport returns a one-time confirmation token;
  confirmation consumes it and never accepts a raw path.
- Refuse permanent mode. A missing recoverable backend disables deletion instead of
  falling back to an irreversible command.
- Append deletion and undo events to `operations.jsonl`. A corrupt line is ignored on
  reload without hiding valid neighboring entries.
- Do not bundle rip. Detection tells users whether a supported installation is available.

## Consequences

Step 10 is complete without pretending the scanner backends can delete. Backend selection
is resolved per ability: ncdu or gdu may scan while rip performs a confirmed deletion.

Windows ships honestly without selected-path deletion until a backend with equivalent
non-interactive recovery and exact undo exists there. Cross-platform does not mean every
capability is fabricated on every platform.

rip has no dry-run mode, so the shell always requires explicit confirmation. The
confirmation summary describes the single validated target but is not labeled as backend
preview output.

The operation journal is Nirmoka data, not a transcription of rip's source or safety
tables. rip remains a separate GPL process and is credited in `NOTICE.md`.
