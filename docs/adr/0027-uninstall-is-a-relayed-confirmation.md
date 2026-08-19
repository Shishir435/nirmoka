# ADR 0027: Uninstall is a relayed confirmation, not a bypassed one

- Status: accepted
- Date: 2026-08-12
- Supersedes [ADR 0021](0021-application-uninstall-is-not-an-adapter-api.md)

## Context

ADR 0021 withdrew application uninstall entirely. Its central factual claim was that
Mole's leftover plan sits _behind_ its confirmation prompt and is therefore unreachable:

> So the leftover plan is _behind_ the prompt, not before it. There is no invocation that
> produces a preview without also asking to proceed.

That is true and it is not the whole picture. ADR 0021 probed `mo uninstall --dry-run <app>`
with **stdin closed**, saw it abort, and concluded the door was shut. Re-probed with `y` on
stdin against the same Mole 1.48.1, recorded in `fixtures/mole/1.48.1/uninstall-plan.txt`:

- `printf 'y\n' | mo uninstall --dry-run <name>` exits **0** and prints the complete plan —
  every leftover path, the backend's own rounded size per path, its `System:` and
  `Review only:` classifications, and its notes about what will survive the removal.
- Under `--dry-run` that prompt **guards nothing**. `MOLE_DRY_RUN=1` is exported during flag
  parsing in `bin/uninstall.sh`, before any discovery, and every destructive call below it is
  separately gated on it (`lib/uninstall/batch.sh` lines 381, 394, 966, 1639). The prompt is
  positioned ahead of a code path that, in this mode, cannot modify a file.
- ADR 0021 never saw the **second** gate. After the plan prints, `batch.sh:938` asks
  `Enter confirm, ESC cancel` through `read -r -s -n1`. That is the real execution gate, and
  it sits _after_ the plan — precisely the shape ADR 0021 assumed did not exist. It treats end
  of input as confirmation, so closing the pipe answers it.
- Mole is already built to be driven from a GUI. `lib/core/sudo.sh:93` detects the absence of
  a TTY and puts up a native macOS password dialog through `osascript`, authenticating the
  user itself. Nirmoka never sees a password.
- Trash routing is the default (`bin/uninstall.sh:1435`); `--permanent` is the opt-in.

So the premise changed in the way that matters. ADR 0021 reasoned that a confirmation dialog
in Nirmoka would be _less_ informed than Mole's own prompt, and replacing a safety gate with a
worse one is not something an adapter may do. But Mole's first prompt shows only a name, a
size, and a last-used date — the plan comes after it. A dialog built on the dry run shows
strictly more: the actual paths, including the ones Mole says it will not touch.

## Decision

Nirmoka offers application uninstall in the window, in two phases with the user's approval
between them.

**Preview.** `mo uninstall --dry-run <name>`, with `y\n` written to stdin. Nothing is modified,
so nothing is bypassed. The adapter parses the plan for rendering and keeps the backend's
verbatim transcript beside it.

**Execution.** Only after the user approves that exact plan: `mo uninstall <name>`, with the
same one line of input. Mole rediscovers every path, applies its own protections, moves the
result to the Trash, and requests administrator authorization itself when a cask or a system
application needs it.

`Capabilities::uninstall_apps` becomes true for Mole. The `Adapter` trait gains
`uninstall_preview` and `execute_uninstall`, and the window only offers the operation when the
backend declares **both** `uninstall_apps` and `dry_run` — the preview is the entire basis on
which the removal is approved, so a backend that could remove without previewing gets the
Terminal handoff rather than a button.

What makes this a relayed confirmation rather than a bypassed one is the ordering, and each
step is enforced rather than documented:

1. A plan cannot be skipped. The confirmation token is issued by `prepare_uninstall`, which
   fails unless a fresh non-empty plan is in hand, and the token is the only thing
   `confirm_uninstall` accepts. No application name and no path is an execute parameter.
2. The plan the user approves is the backend's own output, transcript included, so a parser
   that narrowed it stays visible instead of becoming the whole story. The parser refuses
   rather than guesses: a missing match header, a declared count that disagrees with the list,
   a missing file section, or a missing terminal summary is `MalformedBackendOutput`.
3. Only an identifier the backend itself just published can become an argument.
   `validated_names` checks every name against a live `mo uninstall --list` at preview _and_
   again at execution. This is the adapter boundary doing its job, and it makes "what if a
   name is really a flag" unaskable rather than answered with a denylist — Mole cannot list an
   application whose `uninstall_name` is `--permanent`. It also fails closed on the case that
   matters most: an application renamed, updated, or already removed between review and
   execution is no longer listed, so the run is refused instead of matching something else.
4. `--permanent` is never passed. The user approved a recoverable operation, and a test
   asserts on the recorded argv rather than on a promise in a comment.
5. The reviewed version is bound to the token. A Mole that changed between review and
   execution is `BackendVersionChanged`, and the review has to be redone.
6. Cancellation kills the subprocess, and a cancelled or failed run is an outcome to journal
   rather than an error to raise — files may already have moved.

Every run is journalled beside the cleanup runs from [ADR 0020](0020-cleanup-runs-are-journalled-without-a-receipt.md),
with what was approved and what the backend reported it did. The transcript is deliberately
**not** journalled: it can name every path in a user's library, and the journal is a durable
plaintext file.

## Consequences

The macOS beta closes the loop for both workflows ADR 0019 asked for. Find a problem, review
an exact backend-produced plan, approve it, run it, read the result — now for cleanup and for
uninstall.

Two dependencies on Mole's behaviour are load-bearing, and both are recorded rather than
remembered. The first prompt reads a line, and the second treats end of input as confirmation.
If Mole added a third prompt after the plan, the closed pipe would answer it the same way. That
is the real fragility here and it is worth naming plainly. What bounds it: the version gate is
`>=1.48, <2.0`, `scripts/record-mole-fixture.sh` re-records the plan and the command surface on
every upgrade, and `the_recorded_surface_still_prompts_and_still_defaults_to_the_trash` fails if
Trash routing stops being the default — the one drift that would silently make an approved
uninstall unrecoverable. A release that changes this flow has to be re-derived before the gate
widens.

Uninstall is still not something Nirmoka can do by itself, and this ADR does not move it any
closer. No protected-path list, no cleanup target table, and no leftover-discovery heuristic
enters this repo; Mole's `should_protect_path()` and its curated lists stay Mole's, which is
both the safety position and the licensing one (`NOTICE.md`).

ADR 0021's capability split survives intact and turned out to be the right seam. `app_inventory`
and `uninstall_apps` remain separate flags, which is exactly what let this land as a flag flip
plus two trait methods rather than a restructuring — and what would let a different backend
provide the removal while Mole keeps the inventory.
