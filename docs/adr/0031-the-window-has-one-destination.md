# ADR 0031: The window has one destination

- Status: accepted
- Date: 2026-08-20
- Supersedes the navigation decision in [ADR 0026](0026-the-window-has-three-destinations.md)

## Context

[ADR 0026](0026-the-window-has-three-destinations.md) cut seven nav items to three — Storage, Clean,
Activity — and it was the right cut at the time. Seven entries for three jobs was the fault, and
three destinations fixed it.

The approved design goes one step further, and it does so across all five screens: **there is no
navigation anywhere in it.** The dashboard is the window. An application opens by clicking its row,
and every screen below the dashboard carries `‹ Nirmoka` back to it. Cleanup is reached from the
banner that says how much there is to reclaim, and from the application whose caches would be
cleaned. Nothing is reached by picking it off a list of places.

That is not a style preference. A sidebar is an answer to "where do you want to go", and this
product's user did not open it to go somewhere — they opened it because a disk is full. The
dashboard answers that question directly, and every other screen in the design is a consequence of
something on it. A permanent rail advertising two other destinations is three answers to a question
with one.

ADR 0026's reasoning survives intact, which is why this supersedes only its navigation. Five of the
seven original tabs were filters over one scan, and they are still not places. What changes is that
the remaining three are not places either.

## Decision

**The dashboard is the root, and it is the only one.** The window opens on it, and returns to it.

**Everything else is drilled into and backs out.** The tree browser, the application Inspector, and
the cleanup review all render as their own screen with `‹ Nirmoka` in the top left, exactly as the
design draws it. The `Location` model in `route.ts` keeps its hashes so a bookmark still works; what
goes is the rail that listed them.

**Clean is reached from the thing it acts on.** The reclaimable banner on the dashboard, and the
Inspector for an application whose caches it would remove. A cleanup is a response to a number
somebody just read, not an errand.

**Activity is reached from Settings.** It is a record rather than a destination: it answers "what
did this program do to my disk", which is a question asked after the fact and rarely. The gear opens
a sheet holding the backend choice, appearance, and the way through to Activity — the sheet being
where anything that is neither the disk nor an action on it now lives. Activity itself stays a full
screen with its own back control rather than being folded into the sheet, because a merged timeline
of three journals is a thing to read, and a dialog is not where reading happens.

**The header is the whole chrome.** Window controls, the application's name, and three controls:
Scan, Settings, Help. Nothing else is permanent.

## Consequences

The dashboard gets the full width of the window, which is what the design's two-column layout needs
and what a 56px rail was quietly taking.

Activity gets harder to find, and that is the real cost of this decision. It is a feature that took
work — three journals merged into one timeline, recovery reported per kind — and it is now two
clicks behind a gear. The judgement is that a user who wants it knows they want it, and a user who
does not was paying for it on every screen.

The back control has to be right, because it is now the only way out of anything. `‹ Nirmoka` is a
destination rather than a history step: it returns to the dashboard rather than unwinding wherever
the user came from, so it cannot strand anyone in a loop. Where a screen has depth of its own — the
tree browser, drilling through directories — that depth keeps its own breadcrumb, which is the
design's own answer on screen 5.

What this does not change: no IPC command was added, removed, or widened, no `Capabilities` flag
moved, and the tree still lives in Rust with the window receiving windows of rows. Like ADR 0026,
this is an arrangement of the same surface.
