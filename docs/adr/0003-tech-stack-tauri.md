# ADR 0003 — Build the desktop app with Tauri v2

- **Status:** Accepted
- **Date:** 2026-07-31

## Context

Nirmoka must run on macOS, Linux, and Windows from one codebase. The workload is
specific and modest: spawn a subprocess, stream and parse its stdout, render a large tree
with fast interaction, and handle deletion confirmations. It is not compute-heavy and not
graphics-heavy.

Candidates considered: SwiftUI, React Native, Flutter, Electron, Tauri v2, Wails v3.

## Decision

**Tauri v2**, with a Rust core and a React + TypeScript frontend.

## Rationale

Binary size is a product argument here, not a technical preference. An app whose purpose
is reclaiming disk space cannot credibly ship as a 150 MB download. Tauri uses the
operating system's existing webview, producing single-digit-megabyte binaries. Electron
bundles Chromium and does not.

The Rust surface required is small and well-bounded — spawn, stream, parse, emit — which
makes it a realistic first substantial Rust project rather than an open-ended one. The
adapter trait is the most interesting part and does not demand advanced lifetime work.

Using a web frontend means interface effort goes into a stack that is already familiar,
which matters because interface quality is the entire differentiator.

## Rejected alternatives

**SwiftUI.** macOS only. SwiftUI on Windows and Linux is not a real target. This defeats
the project's primary goal outright.

**React Native.** Desktop support exists as `react-native-macos` and `react-native-windows`,
maintained separately, with no Linux story. Wrong tool for a desktop-first app.

**Flutter.** Genuinely viable — real desktop support, excellent control over animation and
polish, one codebase. Rejected on three counts: Dart is a new language with no transferable
value to this project's backend work, ~40 MB binaries are better than Electron but far
worse than Tauri, and its custom-drawn widgets look identical everywhere rather than native
anywhere. Closest runner-up.

**Electron.** Fastest path to something on screen, familiar stack, trivial subprocess
handling via `child_process`. Rejected on binary size and memory footprint, which
contradict the product's premise.

**Wails v3.** Go instead of Rust, otherwise the same architecture. A reasonable fallback if
Rust proves to be a blocker. Rejected for now: smaller ecosystem, less mature v3, and Rust
is the more valuable thing to learn from this project.

## Consequences

**Good**

- Small downloads, low memory, three platforms from one repo.
- Rust core aligns with the adapter architecture.
- Frontend skills transfer directly.

**Bad**

- Rust has a real learning curve, and this is a learning project. Development will be
  slower at the start.
- Webview inconsistency across platforms — WebKitGTK on Linux behaves differently from
  WebView2 on Windows. Requires testing on all three, not just the development machine.
- Tauri's ecosystem is smaller than Electron's; some problems will need first-party
  solutions.

**Mitigation**

Because the adapter boundary is a process boundary, product logic in `core/` survives a
frontend change. If Tauri proves wrong, moving to Electron or Wails is a contained
migration rather than a rewrite. Starting with Electron and porting later was considered
and rejected only because the port would still cost real time that is better spent
learning Rust now.
