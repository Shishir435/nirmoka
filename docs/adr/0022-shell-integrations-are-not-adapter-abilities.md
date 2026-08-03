# ADR 0022: Shell integrations are not adapter abilities

- Status: accepted
- Date: 2026-08-04

## Context

Consumer navigation needs two things that look like backend features and are not: Reveal in Finder,
and Quick Look. Both take a path from a scan and hand it to the operating system.

The adapter layer was the obvious home, because that is where platform-specific behaviour lives
under invariant 3. It is the wrong home. "Which backend reveals a file" has no meaningful answer —
ncdu, gdu, and Mole would all shell out to the same `open -R`, so the flag would be true for every
adapter that bothered to implement it and the resolution machinery would be picking between
identical implementations. Worse, a `Capabilities` flag describes what a _detected binary_ can do,
and Reveal works when no backend is installed at all.

There is a second question hiding in this one: what does the frontend send. A reveal that takes a
path means the webview holds paths and hands them back for a subprocess argument, which is the
shape the deletion rules exist to prevent.

## Decision

Reveal and Quick Look live in `crates/app`, in `reveal.rs`, beside the window rather than behind
the adapter trait. They are not `Capabilities` flags and take no part in backend resolution.

What the platform can do is reported by `platform_features`, as data: a `revealLabel` and a
`quickLook` flag. The label travels with the flag because each desktop has its own word for the
action — "Reveal in Finder", "Show in File Explorer", "Show in file manager" — and a macOS phrase
on another platform is a macOS assumption leaking out. Quick Look is claimed on macOS only; naming
a Linux or Windows equivalent would mean picking one that may not be installed.

The commands take the scan id and node id, never a path. Rust resolves the path from the tree that
issued the id, checks it still exists, and passes it as an argument rather than through a shell. A
path assembled in the frontend is a path Rust cannot vouch for.

Platform selection uses `std::env::consts::OS` for the reported features, so every platform's
labels are testable from every platform, and `#[cfg]` only for the code that actually differs per
target — the subprocess to spawn.

## Consequences

`Capabilities` keeps describing exactly one thing: what an external disk tool can be asked to do.
A future "open in terminal" or "copy path" lands here too, not as another flag.

A stale row cannot open the wrong file. Between a scan and a click the filesystem may have moved
on, so the path is canonicalised and checked; a path that is gone is an error the row reports
rather than a file manager opening on a reused name.

Linux reveals the containing folder without highlighting the entry, because `xdg-open` has no
select argument. That is less than macOS does, and it is what the platform offers — the flag says
the action exists, the label says what it is called, and neither claims highlighting.
