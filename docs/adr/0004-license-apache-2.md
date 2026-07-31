# ADR 0004 — License Nirmoka under Apache-2.0

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

Nirmoka drives Mole, which is licensed GPL-3.0. The first question is whether Nirmoka is
obliged to be GPL-3.0 as well.

It is not. Nirmoka invokes backends as **separate processes** and exchanges data over
stdout in a documented format. No backend code is copied, linked, or redistributed. This
is arm's-length invocation between independent programs, so GPL-3.0's copyleft does not
reach Nirmoka's source. Nirmoka's license is therefore a free choice.

Two boundaries make that true, and both must be maintained:

1. **No linking or vendoring.** Do not import Mole's Go packages or vendor its shell
   libraries.
2. **No copying its data tables.** Mole's curated cleanup target lists and protected-path
   arrays are the valuable part of Mole. Transcribing them into Nirmoka's source — even
   "just as data" — would make Nirmoka a derivative work. Call the backend instead.

Given a free choice, the candidates were MIT, Apache-2.0, and GPL-3.0.

## Decision

**Apache License 2.0.**

## Rationale

Apache-2.0 contains an explicit clause (Section 6) stating that the license does not grant
rights to use the licensor's trademarks. For this project that is the deciding factor: it
puts name protection directly into the license file, rather than depending on a separate
policy document that forks may not read. Mole achieves the same thing through a separate
`TRADEMARK.md`; Apache-2.0 provides it by default.

It also includes an express patent grant, which MIT lacks, and a requirement that modified
files be marked as changed — useful for tracing what a fork altered.

GPL-3.0 was rejected because Nirmoka's goal is broad adoption of a free tool, not
enforcing openness downstream, and because copyleft would discourage exactly the kind of
casual contribution a learning project benefits from.

MIT was a close second and would be perfectly adequate. Apache-2.0 wins on the trademark
clause at the cost of being longer to read.

## Consequences

- `LICENSE` contains the full Apache-2.0 text with the copyright line filled in.
- `NOTICE.md` carries attribution for backend tools and states the trademark position.
- Contributors license their contributions under Apache-2.0 by submitting them
  (Section 5). No separate CLA is needed.
- The "no linking, no copied data tables" rule must be enforced in code review
  permanently, because violating it silently relicenses the project.
