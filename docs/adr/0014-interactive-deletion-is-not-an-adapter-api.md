# ADR 0014: Interactive deletion is not an adapter API

- Status: accepted
- Date: 2026-08-01
- Supersedes: the `delete: true` interpretation for ncdu and Mole in ADR 0012 and earlier
  architecture text

## Context

The roadmap assumed ncdu could scan and delete caller-selected paths. Step 10 requires a
concrete destructive command before designing the `Adapter` method around it.

ncdu 2.8.2 has no such command. `--enable-delete` controls a keybinding in its ncurses
browser; export mode (`-o`) has no delete operation, and imported exports deliberately
disable deletion because they may not describe the live filesystem.

Mole 1.48.1 also does not fill that gap. `mo clean` operates on Mole's curated categories,
and `mo uninstall` accepts application names. Neither command means "remove this path from
the Nirmoka scan tree". Treating either as arbitrary-path deletion would widen what the
backend actually promises.

Nirmoka cannot silently call `std::fs::remove_dir_all`, `rm`, or platform APIs instead.
That would violate the premise that destructive operations are performed by an existing
backend through its own safety path. Driving ncdu by synthesizing terminal keypresses would
be brittle and would make a UI-state mistake destructive.

## Decision

- `Capabilities::delete` means a non-interactive command that removes a caller-selected
  path. ncdu and Mole both declare it `false`.
- Cleanup categories and application uninstall remain separate capabilities. They do not
  imply arbitrary-path deletion.
- Add and test the common deletion validator now. It resolves symlinks, requires absolute
  paths, enforces strict containment below the scan root, refuses the scan root itself,
  and protects operating-system roots.
- Do not add `Adapter::delete` until there is a backend command that can satisfy it. The
  first implementation must call the common validator and then retain the backend's own
  safety checks.

## Consequences

Step 10 cannot yet offer deletion from the tree. The UI remains honest: there is no button
whose implementation bypasses a backend or automates an interactive terminal.

The next backend investigation must treat selected-path deletion as an explicit acceptance
criterion. If no suitable backend exists, changing the product premise requires a separate
decision; it must not happen as an implementation detail inside this step.
