# ADR 0015: gdu is the Windows scanner

- Status: accepted
- Date: 2026-08-01

## Context

ncdu has no supported Windows build, while gdu publishes Windows binaries and exports ncdu
JSON directly. Nirmoka already names gdu first in the Windows backend order; until this
adapter, that default could only fall through to nothing.

The adapter was tested against gdu 5.32.0. Its export header is ncdu format 1.2 and carries
`progname: gdu`, so the existing streaming parser and contract suite accept it without a
translation layer.

gdu's scan flags do not completely overlap ncdu's. `--no-cross` expresses
`one_file_system`, but gdu 5.32 has no CACHEDIR.TAG option. Its ignore patterns are Go
regular expressions, while `ScanOptions::exclude` promises ncdu-style globs.

## Decision

- Add `adapter-gdu` and register it in the CLI, Tauri shell, and shared contract suite.
- Accept gdu `>=5.32, <5.33`. A new minor release requires a new recorded fixture before the
  version gate widens.
- Pass an empty platform null device through `--config-file` so a user's gdu configuration
  cannot silently change a Nirmoka scan.
- Map `one_file_system` to `--no-cross`.
- Refuse cache-tag and glob exclusion requests as `Unsupported` rather than approximating
  them with different semantics.
- Report scan capability only. gdu's delete key works inside its interactive terminal UI
  and is not a selected-path command an adapter can safely invoke; ADR 0014 applies.
- Install pinned gdu 5.32.0 on Windows CI and run detection, cancellation tests, contract
  fixtures, and a real directory scan there.

## Consequences

Windows now has the platform-default scanner the backend order promised. Adding gdu does
not widen the wire format, change core, or move the tree into the frontend.

The frontend will eventually need to disable unsupported scan options per resolved scanner.
Until that UI exists, the backend fails closed with a precise error instead of producing a
scan whose omissions do not match the request.

gdu constructs its export after parallel analysis rather than yielding entries throughout
the walk. Nirmoka still parses without buffering a second copy, and cancellation kills the
subprocess, but entry-count progress begins later than it does with ncdu.
