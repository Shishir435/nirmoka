# ADR 0009 — Disk usage is the number; apparent size travels beside it

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

The wire format reports two sizes per entry: `asize`, the length a file claims, and
`dsize`, the space it occupies. They are usually close and occasionally very far apart:

- A sparse disk image claims 4 GB and occupies nothing.
- A one-byte file claims one byte and occupies a 4 KB block.
- A hardlinked file claims and occupies its full size under every one of its names, while
  deleting any one of them frees nothing.
- On APFS, directories occupy zero blocks, so `dsize` is absent from the export entirely.

Nirmoka is a cleanup tool. Every number it shows is implicitly a promise about how much
space something would free.

## Decision

**`own_bytes` and `total_bytes` are disk usage.** Sorting, rollup, and every "space freed"
figure use them.

**`apparent_bytes` is kept per node** so the divergence can be shown rather than hidden, but
it is not rolled up. When a directory view needs an apparent-size total, that is a new
field and a new pass, added when the UI actually offers the toggle.

**A missing `dsize` means zero, not "fall back to `asize`."** That fallback would report a
sparse 4 GB image as 4 GB of reclaimable space.

**Hardlinks are counted under their first occurrence.** Later names for the same
`(device, inode)` get zero bytes and a `hardlink` flag. Counting them all would inflate the
total by space that deleting them does not free.

## Consequences

**Good**

- A directory total means "this much comes back if this goes away", which is the only
  meaning a cleanup tool can defend.
- Sparse files, small files, and hardlinks all behave the way `du` and ncdu behave, so the
  numbers reconcile with the tools users already trust.

**Bad**

- Some entries render as `0 B` — a sparse image, a deduplicated hardlink, an APFS directory
  — which looks wrong until the flag beside it is read. The UI must carry those flags; a
  bare zero is worse than no number. `Node::hardlink`, `Node::read_error`, and
  `Node::excluded` exist for this, and `Node::size_is_partial` distinguishes "this zero is
  correct" from "this zero is incomplete".
- Apparent-size views need work that has not been done yet.

## Notes

Hardlink deduplication is keyed on `(device, inode)`, not on the inode alone. Inode numbers
are only unique per filesystem, and deduplicating across devices would silently under-report
a mounted volume — a lie in the other direction, which is not an improvement.
