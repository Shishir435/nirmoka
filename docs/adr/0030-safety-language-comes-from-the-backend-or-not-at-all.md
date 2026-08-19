# ADR 0030: Safety language comes from the backend or not at all

- Status: accepted
- Date: 2026-08-20

## Context

The approved cleanup review screen labels every item with a green **Safe** badge and a reason:

- Unused images — "Dangling images not used by any containers." / "Can be re-downloaded when needed."
- Build cache — "Temporary build cache created by Docker." / "New cache will be created
  automatically."
- Old logs — "Log files older than 7 days." / "Logs are no longer needed for diagnostics."

Nirmoka knows none of that. `CleanupItem` is `path`, `reported_size`, `item_count`. Mole's cleanup
preview is a list of paths under category headings — "Browser caches", "Developer caches" — with a
size comment and no rationale anywhere in the file. Every sentence in that mockup would be written by
us, about a path we did not select, on a screen whose next button deletes it.

Writing them has two costs. The first is that the claims are ours: "can be re-downloaded" is a
promise about someone else's software, made by a program that did not check. The second is worse.
The obvious way to write forty of these is to read Mole's cleanup source and describe what each
target is, and Mole is GPL-3.0. `NOTICE.md` exists because transcribing its curated lists — even as
data, even as prose derived from them — relicenses this project.

## Decision

**Every safety statement on screen is either quoted from the backend or absent.**

**Mole's category name is the label.** "Browser caches" is what Mole calls it and it is the honest
answer to what an item is. It goes where the mockup puts the item title.

**Per-item badges and rationale lines are dropped.** The row keeps its icon, name, path, size and
item count — the design's row minus two sentences.

**One banner carries the safety claim, and it describes the run rather than the item.** What is true
of the whole preview is that Mole selected these paths under its own protected-path rules, that
Nirmoka added nothing to the list, and that the files go to the Trash. Three facts, all checkable,
stated once.

**Attribution to an application is allowed, because it is arithmetic.** Mole's cleanup paths carry
bundle identifiers — `~/Library/Caches/com.example.browser` — so matching a path prefix against a
known bundle id reports which application a cleanup item belongs to. That is a string comparison on
data Mole publishes, not a judgement about what is safe. It is what makes the Inspector's
"reclaimable" panel possible without any rules of our own.

## Consequences

The Review Cleanup screen keeps its structure, its selection model, its totals and its exact-amount
destructive button, and loses the green badges. It reads as a manifest rather than a reassurance.
For a screen whose button is irreversible, a manifest is the better genre.

This also settles a class of future arguments in advance. Whenever a screen wants to tell the user
that something is safe, the question is which program determined that and whether it said so. If the
answer is that Nirmoka inferred it, the sentence does not ship.

The stated cost is that the product is less comforting than the mockup. The uncomforting version is
the one where the reassurance means something.
