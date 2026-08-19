# ADR 0029: The uninstall sheet offers no keep-user-data choice

- Status: accepted
- Date: 2026-08-20

## Context

The approved design's uninstall sheet offers two options, one of them marked Recommended:

- **Remove everything** — 47.2 GB will be freed
- **Keep user data** — remove the application but keep volumes, settings and other user data;
  5.8 GB will be freed

It is a good control. A user who is reinstalling tomorrow wants the second one, and being asked is
better than being surprised.

Mole does not have it. The recorded command surface for 1.48.1 is the whole flag list:

```
--list            List installed apps with the exact name mo uninstall accepts
--dry-run         Preview app uninstallation without making changes
--permanent       Bypass macOS Trash and rm -rf immediately
--whitelist       Not supported for uninstall (use clean/optimize)
--debug           Show detailed operation logs
```

`--whitelist` is the flag shaped like the answer and Mole declines it by name for this subcommand.
There is one uninstall and it removes what it decided to remove. The dry-run plan lists those paths
with sizes, so Nirmoka can _show_ the split perfectly. It cannot _act_ on it.

Which leaves three ways to build the control, and two of them are worse than not having it. Offering
the radio and running the same command either way is a lie the size of 41 GB. Offering it and having
Nirmoka delete the subset itself means Nirmoka assembling a delete command out of paths it parsed
from another program's preview — the exact construction ADR 0014 and ADR 0021 refused, in the one
place where being wrong is unrecoverable.

## Decision

**The sheet shows Mole's plan and offers Cancel or Uninstall.** No radio, no recommendation, no
second number.

**The plan is grouped and totalled, because that part is real.** `mo uninstall --dry-run` prints each
path with its size, and grouping those paths by their `~/Library` location gives the design's
left-hand column exactly as drawn — Application, Containers, Caches, Logs, Other — under an honest
total.

**What Mole will not remove is stated where the design puts the radio.** The dry run is explicit
about its own limits; the Local Network permissions note in the recorded fixture is one. That is
real information about what survives an uninstall, it occupies the same corner of the screen, and it
was written by the program doing the removing.

**Files go to the Trash.** Nirmoka never passes `--permanent`. The strongest available answer to "I
wanted to keep some of that" is that it is still there to put back.

## Consequences

The uninstall sheet is the one screen that reads noticeably differently from the mockup. It keeps the
layout, the per-component breakdown, the freed-space total and the destructive confirm; it loses a
choice that no backend can honour.

If the choice is worth having later, it is a step with its own ADR, and the shape is known: filter
Mole's dry-run plan, trash the remainder through `trash.rs`, and own path validation for the subset.
That makes Nirmoka a deletion executor for the first time. It should be decided on those terms, not
arrived at while styling a modal.

This is ADR 0027 applied one level up. That ADR established that uninstall is a confirmation relayed
to Mole. A control that changes what gets removed is not a relay.
