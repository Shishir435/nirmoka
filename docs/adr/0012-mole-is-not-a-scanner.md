# ADR 0012 — Mole is a cleanup backend, not a scanner

- **Status:** Accepted
- **Date:** 2026-07-31
- **Supersedes the step 9 plan in** [roadmap.md](../roadmap.md), not an earlier ADR.

## Context

The roadmap planned `crates/adapter-mole` as Nirmoka's second **scanner**: run `mo analyze
--json`, translate its richer output down into the ncdu wire format, and let the contract
suite prove the abstraction held. Step 9 existed largely to answer one question — _if adding
a second backend requires changing `core`, the trait is wrong, and it is far cheaper to
learn that at step 9 than at step 15._

The question got an answer. It was not the expected one.

**`mo analyze --json` does not produce a tree.** It lists the _direct children_ of one
directory, with recursive sizes, and stops. The analyzer binary accepts exactly one option:

```
Usage of .../libexec/bin/analyze-go:
  -json
        output analysis as JSON instead of TUI
```

No depth, no recursion, no full-export mode. Recorded evidence is committed in
`fixtures/mole/1.48.1/`: `root.json` names a `nested` directory with its full recursive size
of 65537 bytes and none of its contents; `nested.json`, from a _second_ invocation, is where
its children live.

Reconstructing a tree from this would mean one subprocess per directory. A home directory
holds tens of thousands of them, against a backend that walks the entire subtree on every
call — so the same bytes get read once per level of nesting. ncdu walks the whole thing in
one process, and did 2.2M entries in 50 seconds.

Two further observations from the recordings, both pointing the same way: names carry
display formatting (a symlink is recorded as `"link.txt →"`), and paths are echoed back
uncanonicalised. This is a TUI's data source, not an interchange format, and it does not
claim otherwise.

## Decision

**The Mole adapter declares `scan: false` and returns `AdapterError::Unsupported`. ncdu
remains the only scanner. Mole is adapted for what it is genuinely better at than anything
else available: removal that applies its own curated protections.**

Consequences that follow, each of which is enforced rather than documented:

- `Capabilities` is now per backend everywhere it is shown — `RegistryEntry`, `dto::Backend`,
  the CLI's `SCANS` column, and the backend list in the window. A single set of flags for
  "the active backend" described neither backend once the two stopped overlapping.
- `Registry::first_scanner()` replaces `first_usable()` at every call site that is about to
  scan. "Something is installed" and "something can do this" became different questions.
- The contract suite splits along the capability rather than along the backend name.
  `for_each_scanner` runs the scan promises; `a_backend_that_cannot_scan_refuses_every_scan`
  holds the other side, so a `scan: false` that quietly returned an empty tree would fail.
- `fixtures/mole/` is evidence, not input. Nirmoka never parses Mole's analyzer output;
  `crates/adapter-mole/tests/analyzer_shape.rs` asserts the shape so that a future Mole which
  _does_ emit a tree breaks the build rather than leaving this ADR quietly wrong.

## Consequences

**Good**

- The trait survived contact with a second backend without changing. That was step 9's
  actual question, and the answer is yes — `Capabilities` was already the mechanism for
  "this backend cannot do that", and it turned out to cover the headline ability too.
- Nirmoka gets Mole's genuinely hard-to-replace abilities — curated cleanup, app uninstall,
  protected-path enforcement — without pretending it is a disk walker.
- The alternative designs are both worse in ways that would have surfaced late: a tree one
  level deep presented as complete is a wrong answer, and a subprocess per directory is an
  unusable one.

**Bad**

- "Every adapter scans" was a simplifying assumption, and it is gone. Every place that
  reaches for "the backend" now has to say which ability it means. That is four call sites
  today and will be more.
- Mole contributes nothing until step 10 ships deletion. Between now and then it is a
  detected backend that visibly does not drive the browser — which the backend list has to
  explain rather than leave looking broken.
- `Capabilities::MINIMAL` is no longer the floor it was named for. `scan: false, delete:
true` is below it.

## Notes

Mole is GPL-3.0. This adapter drives its CLI and reads its output; it transcribes none of its
data tables. Its `should_protect_path()` and curated cleanup lists are stricter than anything
this project should attempt, and copying them — even "just as data" — would make Nirmoka a
derivative work and silently relicense it. See [NOTICE.md](../../NOTICE.md).

Re-run `./scripts/record-mole-fixture.sh` after a Mole upgrade. If `root.json` starts
containing entries below its immediate children, this ADR is out of date and the decision
should be reopened with a new one.

The version gate is `>=1.48, <2.0` — a lower bound as well as an upper one, because 1.48.1 is
the only version whose output has been recorded. An older 1.x may work; saying so without
having looked would be the guess this project's version-gating rule exists to prevent.
