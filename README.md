# Nirmoka

**A cross-platform desktop GUI for disk analysis and cleanup — see what's eating your disk on macOS, Linux, and Windows.**

Nirmoka is a free, open source desktop app that gives a real graphical interface to the
disk-usage tools power users already trust. Point it at a directory, watch the tree fill in,
sort by size, and reclaim space — without memorising keybindings or parsing `du` output.

> **निर्मोक** (_nirmoka_) is Sanskrit for the skin a snake sheds — the dead layer a living
> system leaves behind. That is exactly what this tool helps you find and remove.

**Status: early development.** Nothing is installable yet. Watch the repo if you want to know
when the first build lands.

---

## Why another disk tool?

Terminal disk analysers are excellent and fast, but they are terminal apps: hard to
share a screenshot of, hard to hand to a colleague, and hard to browse with a mouse.
Existing GUI options are usually single-platform, abandoned, or several hundred megabytes.

Nirmoka aims at a specific gap:

- **Cross-platform from day one.** One app on macOS, Linux, and Windows.
- **Small.** Built with Tauri, so the download is measured in single-digit megabytes.
  A disk cleanup tool that eats 300 MB of disk is a joke.
- **Not a reimplementation.** Nirmoka does not write its own disk scanner. It drives
  proven CLI tools as backends and focuses entirely on being a good interface to them.
- **Safe by default.** Destructive actions are previewable, and deletion routes through
  the backend's own safety rules rather than a fresh, untested one.

## How it works

Nirmoka is a **frontend with swappable backends**. It spawns an existing disk tool as a
subprocess, reads its structured output, and renders it.

```
┌─────────────────────────────────────┐
│  Nirmoka UI  (React + TypeScript)   │
└──────────────────┬──────────────────┘
                   │  domain types
┌──────────────────┴──────────────────┐
│  Core  (Rust) — no backend knowledge│
└──────────────────┬──────────────────┘
                   │  Adapter trait + capability flags
      ┌────────────┼────────────┐
      │            │            │
 ┌────┴────┐  ┌────┴────┐  ┌────┴────┐
 │  mole   │  │  ncdu   │  │   gdu   │
 │ (macOS) │  │  (any)  │  │  (any)  │
 └─────────┘  └─────────┘  └─────────┘
```

Each adapter declares what it can do (`supports_trash`, `supports_dry_run`,
`supports_uninstall`, …) and the UI adapts. If a backend changes, breaks, or goes
unmaintained, swapping it out is a contained change rather than a rewrite.

See [`docs/architecture.md`](docs/architecture.md) for the full design and
[`docs/adapters.md`](docs/adapters.md) for the backend contract.

## Backends

| Backend                                | Platforms             | Role                                                     |
| -------------------------------------- | --------------------- | -------------------------------------------------------- |
| [Mole](https://github.com/tw93/Mole)   | macOS                 | Rich cleanup, app uninstall, protected-path safety rules |
| [ncdu](https://dev.yorhel.nl/ncdu)     | macOS, Linux, BSD     | Baseline scanner                                         |
| [gdu](https://github.com/dundee/gdu)   | macOS, Linux, Windows | Fast parallel scan, ncdu-compatible export               |
| [rip](https://github.com/nivekuil/rip) | macOS, Linux          | Recoverable selected-path deletion and exact undo        |

Nirmoka does not bundle any of these. It detects what you already have installed and
tells you how to get one if you have none.

## Tech stack

- **[Tauri v2](https://tauri.app)** — native shell, OS webview, small binaries
- **Rust** — core, adapter layer, subprocess handling
- **React + TypeScript** — interface
- **ncdu JSON export format** — the wire format every adapter speaks

## Documentation

- [Architecture](docs/architecture.md) — how the layers fit together
- [Adapter contract](docs/adapters.md) — what a backend must provide
- [Monorepo layout](docs/monorepo.md) — two languages, two package managers
- [Development setup](docs/development.md) — prerequisites and daily commands
- [Roadmap](docs/roadmap.md) — the step-by-step tracker
- [Decision records](docs/adr/) — why each major choice was made

## Development

Requires Node ≥ 22, pnpm, Rust (stable), and ncdu 2.x.

```bash
pnpm install
cargo check --workspace --all-targets
cargo test --workspace

pnpm nrmk backends    # detect installed disk backends
pnpm dev              # frontend dev server
```

See [docs/development.md](docs/development.md) for the full setup.

## Contributing

Not yet — the architecture is still moving. Once the adapter contract is stable and the
first backend works end to end, this section will describe how to add a new backend.

Issues and ideas are welcome in the meantime.

## License

Nirmoka is licensed under the [Apache License 2.0](LICENSE).

Nirmoka is an independent project. It communicates with backend tools as separate
processes and includes no code from any of them. See [NOTICE.md](NOTICE.md) for
attribution and the licenses of the tools it drives.
