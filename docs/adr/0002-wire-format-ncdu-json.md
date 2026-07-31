# ADR 0002 — Use ncdu's JSON export as the internal wire format

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

Every adapter must hand `core/` the same shape of data. Three candidate formats:

1. Define a bespoke Nirmoka format.
2. Adopt the richest backend's output — Mole's `mo analyze --json`.
3. Adopt ncdu's documented JSON export format.

The temptation is option 2. Mole produces the most information, so its format appears to
lose the least. That reasoning is backwards. An interface shaped around the _richest_
backend cannot be satisfied by a poorer one — the ncdu adapter would have to invent or
omit fields, and every gap would leak upward into `core/` as a special case. The result is
a Mole GUI with a backend-shaped hole, and the swap only works on paper.

## Decision

**ncdu's JSON export format is the wire format.** Every adapter emits it.

Reasons:

- It is a published, versioned specification rather than an internal detail.
- ncdu and gdu emit it natively, so two of three adapters need no translation.
- It is the _narrowest_ useful format, which forces the abstraction to stay honest.

Mole's adapter translates `mo analyze --json` down into it. Everything Mole can do beyond
the format is exposed through capability flags rather than by widening the format.

## Consequences

**Good**

- Two adapters get their parser for free.
- `core/` is provably able to work with a minimal backend, because the minimal backend is
  what defines the format.
- The format is externally documented, so the parser is testable against a spec rather
  than against one tool's current behaviour.

**Bad**

- Mole's extra information must be carried out of band, alongside the tree rather than
  inside it. This is a real wart.
- Nirmoka is coupled to a format it does not control. Mitigated by pinning the format
  version and recording fixtures per backend version.

## Notes

Capability flags — not format extensions — are how backend differences get expressed. When
a new backend ability appears, the first question is always "can this be a flag?" Widening
the wire format should require a new ADR.
