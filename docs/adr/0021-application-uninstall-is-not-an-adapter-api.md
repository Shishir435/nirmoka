# ADR 0021: Application uninstall is not an adapter API

- Status: accepted
- Date: 2026-08-04
- Supersedes the uninstall portion of [ADR 0019](0019-mole-consumer-operations-before-beta.md)

## Context

ADR 0019 gated the macOS beta on two Mole workflows: cleanup, and application uninstall with
preview, confirmation, execution, and result reporting. Cleanup landed. Uninstall was planned on
the assumption that `mo uninstall --dry-run <name>` produces a plan the adapter could read and
that `mo uninstall <name>` could then be run non-interactively.

Verified against the installed Mole 1.48.1 release, recorded in
`fixtures/mole/1.48.1/uninstall-command-surface.txt`:

- The full flag set is `--list`, `--dry-run` / `-n`, `--permanent`, `--whitelist`, `--debug`.
  There is no `--yes`, no `--force`, and no environment override.
- A named uninstall — with or without `--dry-run` — matches the application, prints the match, and
  then stops at `Proceed with uninstallation? [y/N]` and blocks on a read from stdin. With stdin
  closed it exits 1 and the plan never prints.
- So the leftover plan is _behind_ the prompt, not before it. There is no invocation that produces
  a preview without also asking to proceed.
- `mo uninstall --list` is unaffected. It emits a JSON array when stdout is not a terminal, and it
  includes the exact `uninstall_name` the command accepts.

The only way past the prompt is to write `y` to Mole's stdin. ADR 0019 already ruled that out in
its own words: driving Mole's interactive flow with synthesized input is not an alternative. The
reason is not stylistic. That prompt is Mole's own confirmation boundary, and an adapter that
answers it has removed a safety gate the backend put there and replaced it with a dialog in a
different program.

## Decision

Nirmoka does not offer application uninstall. `Capabilities::uninstall_apps` is false for Mole,
and no `uninstall` method is added to the `Adapter` trait — an API with no possible implementation
would be a promise about a future backend rather than a description of this one.

Listing applications is a separate capability, `Capabilities::app_inventory`, with its own
`Ability::AppInventory`. Mole declares inventory true and uninstall false. One flag covering both
would force a choice between hiding a working inventory and offering a removal that dies at a
prompt.

The Applications page keeps the inventory, shows each application's exact `uninstall_name`
whenever it differs from the display name, and says plainly that uninstall is run in Terminal with
`mo uninstall <name>`, where the user answers Mole's prompt themselves. Nirmoka does not assemble
a per-application command for them; it hands over the backend's identifier and names the command
form once.

The recorded command surface is a test input. `the_recorded_uninstall_surface_offers_no_non_interactive_flag`
fails if a future Mole documents a way past the prompt, and `scripts/record-mole-fixture.sh`
re-records it. This decision is re-checked on upgrade rather than remembered.

## Consequences

The macOS beta closes the loop for cleanup only. That is one complete consumer workflow —
find a storage problem, review an exact backend-produced plan, approve it, run it, read the
result — and not the two ADR 0019 asked for. The beta ships on that basis or waits for upstream;
it does not ship a bypassed prompt.

Trash-by-default, which ADR 0019 listed as a requirement, is satisfied without any code: Mole
already routes to the Trash and `--permanent` is opt-in. Nirmoka never passes it because Nirmoka
never invokes uninstall at all.

If Mole gains a non-interactive uninstall, the work is a new ADR and a real implementation:
capability flip, a trait method, an execution-bound confirmation, and a journal event beside the
cleanup one from [ADR 0020](0020-cleanup-runs-are-journalled-without-a-receipt.md). Nothing in
this decision blocks that; the capability split is exactly the seam it would land on.

A different backend could also fill this gap. The split means such a backend would declare
`uninstall_apps` while Mole keeps `app_inventory`, and resolution already handles a per-ability
answer without either backend pretending to cover the other.
