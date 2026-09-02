# Security Policy

Nirmoka deletes files. That is the whole risk surface, and a bug there costs a user
something they cannot get back — so security reports are read with more urgency than the
project's size might suggest.

## Supported versions

The macOS beta is the only shipped build, and only the latest release is supported. There
is no backporting: the fix goes into the next release, and `brew upgrade` is the upgrade
path.

| Version        | Supported |
| -------------- | --------- |
| latest release | ✅        |
| anything older | ❌        |

## Reporting a vulnerability

**Do not open a public issue for a vulnerability.**

Use GitHub's private vulnerability reporting — the **Report a vulnerability** button under
the repository's **Security** tab. It creates a private advisory only you and the
maintainer can see.

Please include what you have: the version (`nirmoka --version`, or the release you
installed), your macOS version, which backends are installed (`pnpm nrmk backends` or the
Help page), and the smallest sequence of steps that shows the problem. A path that
triggers it is more useful than a description of one.

Expect an acknowledgement within a week. This is a single-maintainer project, so that is a
realistic number rather than an aspirational one; if a week passes with no reply, an issue
saying only "sent a security report, no reply" is a fine nudge and gives nothing away.

## What counts

Worth reporting:

- A path that escapes validation and reaches a delete, trash, or uninstall operation —
  symlink traversal, `..`, a TOCTOU window between validation and execution.
- A confirmation token that can be replayed, or used against a plan other than the one it
  was issued for.
- Anything that makes Nirmoka remove something the user did not approve, or remove more
  than the reviewed plan described.
- Malformed backend output that causes a crash on a delete path, or that is parsed into a
  plausible-but-wrong plan rather than refused.
- A backend binary resolved from somewhere an attacker can write to.

Probably not a vulnerability, but still worth an issue:

- A wrong size, an under-reported total, or a directory shown as empty rather than
  unreadable. These are correctness bugs and they matter, but they do not remove anything.
- Gatekeeper refusing the unsigned `.dmg`. That is expected and documented — releases are
  unsigned until there is a Developer ID certificate, which is why Homebrew is the
  supported install path. See
  [ADR 0024](docs/adr/0024-distribution-is-a-source-built-homebrew-formula.md).

## Design notes for anyone looking

Two things shape where bugs are likely to be, and both are deliberate:

Nirmoka ships **no disk scanner and no deletion engine of its own**. It drives ncdu, gdu,
Mole, and rip as subprocesses. Safety rules belong to those backends — Mole's
protected-path logic is stricter than anything this project should reimplement — so a
vulnerability in Nirmoka is usually about _what it hands a backend_, not about what the
backend then does. A bug in the backend itself belongs upstream, though we would still
like to know.

Selected-path deletion is **deliberately unavailable**. It was withdrawn in
[ADR 0017](docs/adr/0017-rip-deletion-is-not-execution-bound.md) over a pathname race that
could not be closed against the backends available, and
[ADR 0018](docs/adr/0018-selected-path-deletion-is-deferred-for-v0-1.md) keeps it
closed until a backend can bind execution to the validated object rather than to its name.
Move to Trash is a platform integration, not an adapter capability
([ADR 0025](docs/adr/0025-move-to-trash-is-a-platform-integration.md)). If you find a way
to reach arbitrary deletion through the UI or the IPC boundary, that is exactly the bug
this policy exists for.
