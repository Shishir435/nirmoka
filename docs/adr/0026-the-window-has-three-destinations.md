# ADR 0026: The window has three destinations

- Status: accepted
- Date: 2026-08-06

## Context

0.1.1 shipped seven nav items: Overview, Clean, Space Explorer, Developer, Applications, System
Status, Activity, with Help and Settings beneath them. Every one of them was added for a defensible
reason and the result is still hard to use. The first thing a beta user said about it was that the
tabs were confusing.

The reason is that the nav grew one entry per **command surface** rather than one per user intent.
Five of the seven read from the same completed scan:

- Overview — summary metrics, a donut, and the eight largest entries, ending in an "Explore" button
  that navigates to Space Explorer
- Space Explorer — the same scan, as a tree
- Developer — `developer_inventory(scanId)`: the same tree, filtered to Xcode, git, and node_modules
  evidence
- Applications — `application_inventory(scanId)`: the same tree, filtered to `.app` bundles, beside
  a second list from Mole that follows different rules
- System Status — `mo status`: not the scan at all, and not disk cleanup either

So a user scans on one tab and drills on another, both of which mount their own `ScanControls` even
though Rust holds exactly one scan. Two of the seven are filters presented as places.

Two smaller faults come from the same shape. `activity-page.tsx` read only `operation_log()`, so a
file moved to the Trash appeared in no history at all while cleanup history sat inline on the Clean
page — three journals, one of them shown, one of them shown somewhere else, one of them nowhere. And
the sidebar carried a "Read Only Mode" badge that was already half wrong in 0.1.1, because Clean
executes real Mole cleanups, and fully wrong once Trash landed.

## Decision

Three destinations: **Storage**, **Clean**, **Activity**. Help and Settings are header controls, not
nav items.

**Storage owns everything derived from the scan tree.** One scan bar, in the shell header, because
the scan is one process-wide thing in Rust and having it in two places implied it was two. Below it
the summary, and then a view switch over the same tree: Folders, Developer, Applications. `Location`
carries `route` and `view` separately, so the presets are views of one page rather than routes that
happen to share a data source.

**Retired hashes redirect.** `#/overview`, `#/space`, and `#/status` resolve to Storage; `#/developer`
and `#/applications` resolve to Storage with that view selected. A bookmark from 0.1.1 lands on the
content it named instead of falling through to a default, and `locationFromHash` is a pure function
in `route.ts` with the redirects under test.

**Activity is the one history.** `mergeActivity` in `activity-feed.ts` merges all three journals into
one timeline, newest first, breaking ties on the id — exact rather than arbitrary, because Rust
issues trash, cleanup, and deletion ids from one counter. Recovery is reported per kind and none of
the three is a button: the Trash is restored by the Finder, a cleanup run has no per-path receipt to
restore from, and only a recorded recoverable deletion can be undone through a backend.

**System status stays, lower.** It is a section on Storage that loads when opened, not a tab and not
an automatic `mo status` on every visit.

**The badge says what is true.** Nothing is removed without confirmation, removal is recoverable, and
what remains unavailable is _permanent_ selected-path deletion — which is ADR 0017's gate, not a mode.

## Consequences

`overview-page.tsx`, `space-page.tsx`, `developer-page.tsx`, `applications-page.tsx`, and
`status-page.tsx` stop being routes and become sections under `pages/sections/`. Their content is
unchanged: this ADR moves screens and deletes nothing a backend reported.

Two more pure modules join the ones step 11 phase 5 extracted — `route.ts` and `activity-feed.ts` —
so redirect behaviour and timeline ordering are tested by `node --test` rather than by clicking
through the window.

The cost is that Storage is now the page that can grow badly. A sixth view would be the same mistake
at one level down, and the answer then is the same as it was here: a view is a filter over one scan,
and anything that is not that does not belong on it.

What this does not change: no IPC command was added, removed, or widened, no `Capabilities` flag
moved, and the tree still lives in Rust with the window receiving windows of rows. This is an
arrangement of the same surface.
