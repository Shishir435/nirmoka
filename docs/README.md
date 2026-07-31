# Nirmoka Documentation

Design documentation for Nirmoka, a cross-platform desktop GUI for disk analysis and
cleanup.

## Start here

| Document | What it covers |
|---|---|
| [Architecture](architecture.md) | The layers, the rules that keep them separate, and why |
| [Adapter contract](adapters.md) | What a backend must provide to be usable |
| [Roadmap](roadmap.md) | Planned milestones, in dependency order, plus what is out of scope |

## Decision records

Each ADR records one significant choice, the alternatives that were rejected, and the
consequences accepted along with it. They are written to be readable by someone who was
not in the room.

| ADR | Decision |
|---|---|
| [0001](adr/0001-adapter-pattern.md) | Drive existing CLI tools through an adapter layer |
| [0002](adr/0002-wire-format-ncdu-json.md) | Use ncdu's JSON export as the internal wire format |
| [0003](adr/0003-tech-stack-tauri.md) | Build the desktop app with Tauri v2 |
| [0004](adr/0004-license-apache-2.md) | License Nirmoka under Apache-2.0 |

## The three ideas that matter

If you read nothing else:

**Nirmoka writes no disk scanner.** It drives proven command-line tools as subprocesses.
The gap in the market is interface quality, not scan speed, and a side project cannot
reproduce years of curated knowledge about which paths are safe to delete.

**The wire format is the narrowest backend's, not the richest.** Building against ncdu's
export format rather than Mole's richer output is what keeps the abstraction honest.
Backends' extra abilities are capability flags, never format extensions.

**Deletion is the whole risk surface.** Paths are validated at the adapter boundary before
they ever become subprocess arguments, backends' own safety rules are called rather than
reimplemented, and no adapter fakes a preview it cannot actually produce.

## Conventions

- ADRs are numbered sequentially and never deleted. A reversed decision gets a new ADR
  marking the old one superseded.
- New backend abilities are capability flags by default. Widening the wire format requires
  its own ADR.
- Anything decided in an issue thread that will matter in six months belongs here instead.
