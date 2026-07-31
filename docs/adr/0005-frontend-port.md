# ADR 0005 — Keep the frontend replaceable with a second port

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

[ADR 0001](0001-adapter-pattern.md) makes backends swappable through an adapter trait.
That protects against one class of risk — an upstream disk tool breaking, going
unmaintained, or not existing on a platform.

It does nothing about the symmetric risk on the other side: Tauri. The question raised
was, plainly, "if Tauri becomes a problem, do we have an answer?"

Decomposing "Tauri becomes a problem" matters, because the failure modes have very
different likelihoods:

| Risk                             | Probability                                   | Impact         |
| -------------------------------- | --------------------------------------------- | -------------- |
| Tauri abandoned                  | Low — well funded, large community, v2 stable | High           |
| Linux WebKitGTK divergence       | **High**                                      | Medium         |
| Rendering ceiling on large trees | High if architected wrong, near zero if right | High           |
| Rust slowing early development   | High                                          | Low, temporary |

The risk people worry about (abandonment) is the least likely. The real one is that Tauri
uses each OS's own webview, so Linux gets WebKitGTK — a genuinely different renderer from
WebView2 and WKWebView, with its own font rendering and compositing behaviour.

## Decision

**`nirmoka-core` is a plain Rust library with no dependency on Tauri or any GUI
framework.** The architecture becomes a hexagon with two ports rather than one:

```
   UI (React)  ──┐
                 ├── [transport port] ── core ── [Adapter trait] ── mo / ncdu / gdu
   nrmk (CLI)  ──┘
```

Three mechanisms make it real rather than aspirational:

**1. `crates/cli` (`nrmk`) links `core` with no Tauri.** If someone adds a `tauri`
dependency to `core`, this binary stops building. The boundary is enforced by the compiler,
not by a paragraph in a design document. See [ADR 0007](0007-nrmk-cli-scope.md).

**2. `crates/app` is a dumb translation layer.** Tauri commands in, core calls out; core
events in, Tauri events out. A few hundred lines with no product logic. Leaving Tauri means
rewriting that crate, not the app.

**3. `packages/transport` is the only TypeScript module allowed to import
`@tauri-apps/*`.** Every component imports from `@nirmoka/transport`. CI greps for
violations. Moving to Electron rewrites one file; moving to a browser-served build rewrites
one file.

## Consequences

**Escape routes and what each actually costs:**

| Escape to              | `core` + adapters                          | UI        | Cost                               |
| ---------------------- | ------------------------------------------ | --------- | ---------------------------------- |
| Electron               | kept — spawn `nrmk` as sidecar, or napi-rs | ~all kept | Low-medium; binary size much worse |
| egui / iced            | kept entirely                              | rewritten | Zero logic loss, full UI rewrite   |
| Local daemon + browser | kept entirely                              | ~all kept | Low; loses native feel             |
| Wails (Go)             | **lost** — rewrite in Go                   | kept      | High. Avoid.                       |

Three of four preserve the expensive part.

**Good**

- The GUI framework is a replaceable component, not a foundation.
- `core` is testable and benchmarkable with no GUI harness.
- CI exercises the whole stack with no display server.
- Worst case is "the project is a working CLI while the GUI is rebuilt", never nothing.

**Bad**

- Two boundaries to maintain discipline across instead of one.
- Some IPC ceremony that a Tauri-native design would skip.
- `nrmk` is a second binary to keep compiling, forever, for a benefit that is invisible
  when everything is working.

## The mitigation that is not architectural

The Linux webview risk cannot be designed away. It is handled by **getting a Linux build
into CI at step 0**, not step 11. Discovering WebKitGTK behaviour after the whole UI exists
is the expensive version of finding out.

Related, and the reason invariant 5 exists: a home directory scan is 500k–2M nodes. Ship
that into JavaScript and render it as DOM and the app will crawl — and the conclusion will
be "Tauri is slow", when Electron would crawl identically. **The tree model stays in Rust;
the webview receives only the visible window plus aggregates.** Most reported "Tauri
performance problems" are this mistake.

## The tripwire

Decided now, while nobody is frustrated:

> If a Tauri-specific defect blocks a roadmap step for more than two weeks with no
> workaround, evaluate the escape routes above. Not before.

Without a stated threshold, "Tauri annoyed me this week" becomes a rewrite at month three.

## Rejected alternative

**An abstraction layer over Tauri's APIs, built preemptively.** Rejected as speculative
generality: it taxes every feature for a risk that probably never lands.

The distinction worth keeping: **the insurance is a boundary, not an abstraction.** `core`
having no `tauri` dependency costs nothing per feature. A `WindowManager` trait with one
implementation costs something every day.
