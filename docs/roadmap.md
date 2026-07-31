# Roadmap

Ordered by dependency, not by excitement. Each milestone should be usable before the next
one starts.

## M0 — Claim and plan (current)

- [x] Name chosen and verified unclaimed on GitHub, npm, crates.io, Vercel, Cloudflare
- [x] License chosen: Apache-2.0
- [x] Attribution written
- [x] Architecture and adapter contract drafted
- [ ] GitHub repository created and linked to Vercel
- [ ] `nirmoka.vercel.app` serving the landing page

## M1 — One backend, end to end

The goal is a window that shows a real directory tree, not a polished one.

- [ ] Tauri v2 project scaffold, workspace with `core` / `adapter` / `adapter-ncdu` / `app`
- [ ] ncdu adapter: detect, version gate, scan, stream
- [ ] ncdu JSON export parser
- [ ] Minimal tree view — name, size, sorted descending
- [ ] Navigate in and out of directories
- [ ] Cancel a running scan

**ncdu first, not Mole.** ncdu is the narrowest backend, so building against it first
prevents Mole's richer output from shaping the interface. This is the single most
important sequencing decision in the project.

## M2 — Prove the abstraction

- [ ] Mole adapter: detect, version gate, translate `mo analyze --json` into ncdu format
- [ ] Capability flags wired through to the UI
- [ ] Backend picker, with auto-detection and a manual override
- [ ] Contract test suite running against both adapters from recorded fixtures
- [ ] A clear, non-scary screen for "no supported backend installed"

If adding the second adapter requires changing `core/`, the trait was wrong. Fix the trait
here, while it is still cheap.

## M3 — Deletion

Nothing in this milestone ships without tests.

- [ ] Path validation at the adapter boundary
- [ ] Dry-run preview where the backend supports it
- [ ] Confirmation flow for backends that do not
- [ ] Trash where available, permanent delete clearly marked where not
- [ ] Undo affordance for trashed items
- [ ] Operation log the user can actually read

## M4 — The part worth caring about

- [ ] Treemap view
- [ ] Keyboard-first navigation, with the shortcuts discoverable
- [ ] Live scan progress that does not lie about how far along it is
- [ ] Empty, loading, error, and permission-denied states designed rather than defaulted
- [ ] Dark and light themes
- [ ] Actually good typography and alignment for size columns

## M5 — Ship

- [ ] Windows via the gdu adapter
- [ ] Signed macOS build
- [ ] Linux AppImage or Flatpak
- [ ] CI: build all three platforms, run contract tests
- [ ] Documentation site fleshed out
- [ ] First tagged release

## Explicitly out of scope

Writing these down now, so they can be declined quickly later:

- **A built-in disk scanner.** The entire premise is that scanning is a solved problem.
- **Bundled backend binaries.** Detect and guide; do not redistribute.
- **Background monitoring, menu bar agents, scheduled cleanups.** This is a tool you open
  when you need it.
- **Reimplementing any backend's curated cleanup lists.** Legally risky and immediately
  stale.
- **Mobile.** Tauri can technically target it. There is no disk to browse there.
