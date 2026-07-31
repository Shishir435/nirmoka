# ADR 0008 — The wire-format parser lives in `crates/adapter`

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

[ADR 0002](0002-wire-format-ncdu-json.md) makes ncdu's JSON export the format every adapter
emits. Step 4 needed a parser for it, and there were three places it could go.

**`crates/core`.** The natural-looking home, since the parser produces a `Tree`. Ruled out
by invariant 1: core depends on the standard library, serde, and thiserror, and nothing
else. A JSON parser is a fourth dependency, and — worse — it would make the domain model
know the shape of one backend's output.

**`crates/adapter-ncdu`.** Where the format came from, so this looks obvious. But the
format is not ncdu's private business: gdu emits it natively, and Mole's adapter translates
down into it. Both would then have to depend on `adapter-ncdu` to parse a format neither
gets from ncdu. Adapters are supposed to be independent and individually removable; making
two of them depend on a third makes the ncdu adapter load-bearing for backends that have
nothing to do with ncdu.

**`crates/adapter`.** The crate that already holds the trait, the capability flags, and the
detection types.

## Decision

**The parser lives in `crates/adapter`, as `wire`.**

The wire format is part of the adapter contract, in the same way the `Adapter` trait is. An
adapter's obligation is "emit this shape"; the crate that states the obligation is the
crate that should be able to read it.

`crates/adapter` gains a `serde_json` dependency. This is allowed — invariant 1 constrains
`core`, not the contract crate — and the reason is recorded here rather than left to be
rediscovered.

## Consequences

**Good**

- `adapter-gdu` and `adapter-mole` parse the format without depending on `adapter-ncdu`.
- The step 6 contract suite parses fixtures from _every_ backend with one parser, which is
  what makes "the same suite runs against every adapter" true rather than aspirational.
- `crates/core` stays at three dependencies and knows nothing about JSON.

**Bad**

- The contract crate is no longer purely a set of interfaces. It carries an implementation,
  and that implementation is a parser with real complexity in it.
- A second wire format — if one is ever accepted, which needs its own ADR — would sit
  awkwardly beside this one.

## Notes

The parser streams into a `WireSink` rather than returning a document. Buffering the whole
export before handing anything over would break the promise in `docs/adapters.md` that the
UI can paint its first rows while the backend is still walking the disk, and it would hold
tens of megabytes of JSON text alongside the tree it is being turned into.

`TreeSink` — the sink that produces a `nirmoka_core::Tree` — lives beside the parser for the
same reason: every adapter needs it, and no adapter should own it.
