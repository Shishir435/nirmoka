# ADR 0013 — The backend is a choice, resolved per ability

- **Status:** Accepted
- **Date:** 2026-07-31
- **Builds on** [ADR 0012](0012-mole-is-not-a-scanner.md).
- **Extended by** [ADR 0016](0016-rip-is-the-selected-path-deletion-backend.md), then
  [ADR 0017](0017-rip-deletion-is-not-execution-bound.md).

## Context

Until now, which backend ran was decided by **registration order**: `main` pushed ncdu
first, Mole second, and the first usable one won. That worked while there was one backend,
and survived the second only because [`Registry::first_scanner`](../adapters.md) filtered on
a capability flag before taking the first match.

Two things break it.

**The right default is not the same on every platform.** Mole is the better tool on macOS —
curated cleanup, app uninstall, protections stricter than anything this project should
write. ncdu is what exists everywhere else. gdu is the realistic Windows scanner. A single
ordering compiled into `main` can only be right on one platform, and it was right on the one
the developer happens to use.

**The user has a preference and no way to say it.** A machine with ncdu, gdu, and Mole
installed has three backends and one opinion about which to use, and registration order
ignored it entirely.

The obvious fix — honour the choice — is wrong on its own. Mole is the macOS default and
**cannot scan** (ADR 0012). "The user picked Mole, so Mole runs" turns a preference into a
disk browser that reports nothing.

## Decision

**Selection is a user preference with a per-platform default, resolved separately for each
ability.** A preference is honoured wherever the backend can do the job, and fallen back
from where it cannot — never silently.

`Registry::resolve(ability, preference)` runs three passes, each filtered by whether the
adapter is usable _and_ declares the ability:

1. **The user's choice.** Honoured whenever it can be.
2. **The platform default order** — macOS `mole, rip, ncdu, gdu`; Windows
   `gdu, rip, ncdu, mole`; everywhere else `ncdu, rip, gdu, mole`. rip was added by
   ADR 0016 and capability filtering keeps it out of scans.
3. **Registration order**, for a backend no default names.

It returns a `Choice`, which carries the adapter _and_ `instead_of` — the backend that was
asked for, when it is not the one that will run. That field is the decision's honesty
clause: a fallback that did not name who was displaced would read as the setting being
ignored.

Consequences that follow:

- **Registration order stops being preference order.** It is now the last tiebreak, reached
  only by a backend no default names — kept so that a new adapter is reachable rather than
  invisible before anyone updates the defaults.
- **`gdu` was named in every default order before its adapter existed.** An id matching no
  registered backend is skipped, not an error. Step 11 later added the adapter without
  changing this ordering decision.
- **`None` is a real preference,** not an absent one. It means "follow the platform default"
  and keeps following it when a later release changes the defaults — which a value written
  eagerly on first run would not.
- **The choice is stored** in `settings.json` beside the bundle, via the `directories` crate.
  It is the only thing the shell remembers between launches.
- **`nrmk` does not read that file.** It takes `--backend` and otherwise uses the platform
  default. A harness that inherited a developer's preferences would reproduce their machine
  rather than the default one — see ADR 0007 on why `nrmk` is not a product.

### No `#[cfg(target_os)]`

The defaults are matched on `std::env::consts::OS` at runtime rather than compiled per
target. It costs a string compare that is never on a hot path, and every platform's default
becomes testable from every platform — the Windows ordering is covered by CI on Linux and
macOS, not only by the one job that runs on Windows.

### The reason is not guessed

A preference can go unmet because the backend cannot do the thing, because it is not
installed, or because it is at an untested version. `Choice` reports only _that_ it went
unmet. The CLI prints the fact and points at `nrmk backends`; the window, which already
holds every backend's detection state, reads the reason from there.

Claiming one would eventually tell somebody to install what they already have.

## Consequences

**Good**

- Choosing Mole on macOS is now a coherent thing to do: Mole cleans, uninstalls, and reports
  status, while ncdu still scans — and the window says exactly that rather than appearing to
  ignore the setting.
- The macOS default finally names the tool a macOS user wants, which registration order
  could never do without breaking Linux.
- The capability filter that made this safe already existed. ADR 0012 forced `Capabilities`
  to be per backend; this is what that bought.

**Bad**

- "Which backend is Nirmoka using" no longer has one answer. It has one per ability, and
  every surface that mentions a backend has to say which job it means.
- Resolution runs detection on each candidate, so it is a handful of subprocesses. Fine for a
  button press and wrong for a loop; nothing calls it in one today.
- Before step 11, a default naming an unimplemented backend was a real, if small, trap: `gdu`
  sat first on Windows and correctly resolved to nothing. ADR 0015 closes that gap.

## Notes

Asserted against the binaries in CI rather than left as prose: `nrmk --backend mole backends`
must report `SCANS WITH ncdu` and name the unmet preference, and `nrmk --backend mole scan`
must actually complete on ncdu. Both run on every platform where a usable ncdu exists —
macOS, where Mole is installed and cannot scan, and Linux, where it is not installed at all.
The same two lines have to come out either way.
