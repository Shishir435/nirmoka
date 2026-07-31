# ADR 0011 — Ordering and paging happen in Rust

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

Step 8 is the tree view: a scrollable list of one directory's children, sortable by size and
by name, navigable in and out of subdirectories.

The reflex shape for this in React is to fetch the rows, hold them in state, and sort with
`Array.prototype.sort` in a `useMemo`. Every list tutorial is written that way, and for a
list that fits in memory it is the right answer.

It is the wrong answer here, and the reason is invariant 5 rather than performance. The
frontend never holds a directory — it holds a window onto one. A `node_modules` with 120,000
entries arrives forty rows at a time. Sorting those forty rows produces a list that is
ordered within the window and unordered across the directory, and the bug it creates is
subtle: the screen looks correctly sorted, and the largest file in the directory is not on
it.

The same argument applies to filtering, and will apply to search when it arrives.

## Decision

**The frontend asks for a window; Rust decides what is in it.**

- `Sort` is a request parameter on `rows`, not a client-side transform. It travels as one of
  four named orders — `largestFirst`, `smallestFirst`, `nameAscending`, `nameDescending` —
  each naming both key and direction, because "ascending" has no obvious meaning for a size.
- `Tree::children_sorted` orders the whole directory, then the window is cut from the result.
- Comparisons fall through to the node id, so ties are broken the same way every time. Two
  requests for overlapping windows have to agree on where a row sits, or scrolling past a
  tie shows one entry twice and skips another.
- `RowPage` echoes the sort back. What was asked for and what is on screen are different
  things while a request is in flight, and the controls describe the latter.
- The way back out travels with the page, as `ancestors`. The frontend holds one node id at
  a time; without the chain, "up" would mean rescanning.

## Consequences

**Good**

- Sorting is correct for directories of any size, and correct for the same reason at 40
  entries as at 400,000.
- Re-sorting costs one IPC round trip and no re-scan — the tree is already in memory.
- Ordering stays in `nirmoka-core`, where the CLI and the contract suite reach it. `nrmk
scan` and the GUI cannot disagree about what "largest first" means.
- The frontend's list code has no opinion about ordering at all, which is what makes it
  short.

**Bad**

- Changing a sort is a network-shaped operation with a latency and a failure mode, where
  client-side sorting would have been instant. In practice it is an in-process call behind
  Tauri's IPC, and the page it returns is a few dozen rows.
- Every future ordering option — group directories first, sort by name with numeric
  awareness — is a Rust change plus a regenerated binding, not a one-line comparator in a
  component.

## Notes

The chunk size the frontend requests (`CHUNK` in `use-directory.ts`, 100 rows) is a UI
tuning knob and lives in the UI. `MAX_ROWS` in `crates/app/src/commands.rs` is not: it is
the cap that makes "ask for the whole tree" impossible regardless of what the caller sends,
and it stays in Rust.
