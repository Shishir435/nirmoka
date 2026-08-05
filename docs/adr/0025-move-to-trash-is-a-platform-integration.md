# ADR 0025: move to Trash is a platform integration

- Status: accepted
- Date: 2026-08-05
- Supersedes: [ADR 0018](0018-selected-path-deletion-is-deferred-for-v0-1.md), for recoverable
  removal only

## Context

v0.1.1 ships a disk analyzer that cannot remove anything a user selects. Scanning works on a real
home directory — 2.5M entries — and the loop it opens closes somewhere else: the user finds the
21 GiB directory, presses Reveal in Finder, and deletes it there. Every screen has this shape.
Space browses. Applications lists apps it cannot uninstall. Clean runs Mole's own curated
selection and nothing the user picked.

[ADR 0017](0017-rip-deletion-is-not-execution-bound.md) withdrew rip because its pathname-only
interface cannot be bound to the object Nirmoka validated: another process can replace an ancestor
between the containment check and rip's own resolution.
[ADR 0018](0018-selected-path-deletion-is-deferred-for-v0-1.md) then deferred the whole capability
rather than ship a safety claim no backend could honor, and set the gate for reopening it — a
backend that binds execution to the validated filesystem object, or an equivalent atomic
containment guarantee.

No such backend has appeared, and **the macOS Trash does not meet that gate either.**
`-[NSFileManager trashItemAtURL:resultingItemURL:error:]` takes a URL. It resolves that URL itself,
after Nirmoka's check, so the race ADR 0017 identified survives unchanged. Anyone reading this
looking for the moment the race was closed will not find it.

What changes is the consequence rather than the race. macOS records an item's original location
when it moves it to the Trash, so **Put Back** restores it exactly, from the Finder, with no
involvement from Nirmoka. A lost race puts the *wrong item* in the Trash. It does not destroy it.

That is a weaker guarantee than ADR 0017 asked for and a different kind of guarantee: bounded and
reversible rather than atomic and bound. It is enough for a recoverable move and it is not enough
for permanent removal, which is the line this ADR draws.

## Decision

- **Move to Trash is a platform integration, not an adapter ability.** It lives in `crates/app`
  beside Reveal in Finder and Quick Look, for the reason
  [ADR 0022](0022-shell-integrations-are-not-adapter-abilities.md) already gives: no disk tool is
  involved and the answer depends on the desktop rather than on which scanner is installed.
  "Which backend trashes a file" is a question with no meaningful answer.
- **Every adapter continues to report `delete: false` and `trash: false`.** `Capabilities`
  describes what backends can do. Widening it here would claim an ability whose implementation is
  not in an adapter at all.
- **Trash only. No permanent removal.** `DeleteMode::Permanent` stays unreachable from the window.
  The recoverability argument above is the whole basis for proceeding, so an operation that
  discards it is a different decision needing a different ADR.
- **The shared validator runs, and runs again immediately before the move.** `validate_delete_target`
  is unchanged: absolute, canonical, strictly below the scan root, and outside the protected OS
  roots. Re-validating does not close the race; it closes the stale-confirmation and
  symlink-retarget cases, which are the ones a check *can* close.
- **The one-time confirmation token boundary is reused unchanged.** A raw path never crosses back
  from the window into an execute command.
- **The operation is journalled as `Trashed`, with no recovery path.** The `trash` crate cannot
  enumerate or restore the macOS Trash — its `os_limited` module is compiled out on macOS — and
  Nirmoka must not guess a path inside `~/.Trash`, where macOS renames on collision. Recovery is
  Finder's Put Back, and the window says so rather than offering an Undo button that shells out to
  a guess.
- **A failed journal append reports the move beside the error rather than failing it.** This
  follows [ADR 0020](0020-cleanup-runs-are-journalled-without-a-receipt.md), not ADR 0017's receipt
  rule. Those two rules differ on one question: does recovery depend on our record? For rip it did —
  the receipt was the only route back, so an unwritable journal meant the deletion was not safely
  performed. For the Trash it does not; the Trash is its own record. Hiding a move that already
  happened would lose the only account the user has of it.
- **The move happens first, then the journal write.** The reverse order records removals that did
  not happen, which is the worse of the two failures.

Implementation is the `trash` crate (5.2.6, MIT), not a subprocess. It calls the same platform
Trash service Finder uses; the alternative — `osascript` telling Finder to delete, or a
`brew install trash` dependency — either drives another app's UI scripting layer or makes the
product's central verb depend on an optional install. The crate covers Windows and the freedesktop
specification too, so the code stays platform-neutral under invariant 3 even though
[ADR 0023](0023-the-first-release-is-macos-only.md) packages macOS only.

## Consequences

Nirmoka becomes a tool rather than a viewer. The loop closes in one window: scan, find, review the
exact path and size, confirm once, and the item is in the Trash where it can be put back.

The residual risk is stated rather than resolved. A process that swaps an ancestor at the right
moment can cause the wrong item to be trashed, and no check in Nirmoka prevents that. What bounds
it is that the item is in the Trash.

Permanent selected-path deletion remains deferred under ADR 0017's original gate. Nothing here
weakens it, and a future backend that binds execution to a validated object is still what reopens
it.

A trashed row leaves the scan stale. Sizes above it were computed when the scan ran and are not
recomputed — a rescan is the accurate number, and the window must not quietly redraw a total it
did not measure.
