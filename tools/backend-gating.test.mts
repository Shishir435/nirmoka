import assert from "node:assert/strict";
import test from "node:test";

import {
  cleanupAvailability,
  moleSetup,
  scanAvailability,
  scannerSetup,
  uninstallOffer,
} from "../apps/desktop/src/lib/engine/backend-gating.ts";

const backend = (id: string, usable: boolean, capabilities: Record<string, boolean> = {}) =>
  ({
    id,
    displayName: id,
    supportedVersions: "*",
    detection: usable
      ? { state: "found", path: `/fake/${id}`, version: "1.0.0" }
      : {
          state: "notInstalled",
        },
    error: null,
    usable,
    capabilities: {
      scan: false,
      delete: false,
      trash: false,
      undo: false,
      dryRun: false,
      cleanupCategories: false,
      appInventory: false,
      uninstallApps: false,
      systemStatus: false,
      ...capabilities,
    },
  }) as never;

const mole = (capabilities: Record<string, boolean>, usable = true) =>
  [backend("ncdu", true, { scan: true }), backend("mole", usable, capabilities)] as never[];

const selection = (scanner: string | null) =>
  ({
    chosen: null,
    defaultOrder: ["mole", "rip", "ncdu", "gdu"],
    scanner,
    scannerInsteadOf: null,
    persistent: true,
    saveError: null,
  }) as never;

const unsupported = (id: string, capabilities: Record<string, boolean>, supported: string) =>
  ({
    ...backend(id, false, capabilities),
    detection: {
      state: "unsupportedVersion",
      path: `/fake/${id}`,
      version: "0.1.0",
      supported,
    },
  }) as never;

/** "Nothing is installed" and "we have not looked yet" are different claims. */
test("detection in progress is not reported as a missing backend", () => {
  const pending = scanAvailability(null, null);

  assert.equal(pending.available, false);
  assert.match(pending.reason, /Detecting/);
});

test("a scan needs a resolved scanner, whatever else is installed", () => {
  assert.equal(scanAvailability([], selection(null)).available, false);
  assert.match(scanAvailability([], selection(null)).reason, /No supported scanner/);

  // Mole is usable and cannot scan, so resolution falls back — the page must
  // gate on what resolved, not on whether a backend was found.
  const molePreferred = mole({ cleanupCategories: true, dryRun: true });
  assert.equal(scanAvailability(molePreferred, selection(null)).available, false);
  assert.equal(scanAvailability(molePreferred, selection("ncdu")).available, true);
});

test("scanner setup never asks for Mole and distinguishes install from upgrade", () => {
  assert.equal(scannerSetup(null, null).state, "checking");
  assert.equal(scannerSetup([], null).state, "unavailable");

  const missing = scannerSetup([], selection(null));
  assert.equal(missing.state, "install");
  assert.equal(missing.command, "brew install ncdu");
  assert.doesNotMatch(missing.command, /mole/);

  const old = scannerSetup([unsupported("ncdu", { scan: true }, ">=2.0, <3.0")], selection(null));
  assert.equal(old.state, "upgrade");
  assert.equal(old.command, "brew upgrade ncdu");

  const ready = scannerSetup([backend("ncdu", true, { scan: true })], selection("ncdu"));
  assert.equal(ready.state, "ready");
  assert.equal(ready.command, null);
});

test("Mole setup is optional, contextual, and version-aware", () => {
  assert.equal(moleSetup(null).state, "checking");

  const missing = moleSetup([backend("ncdu", true, { scan: true })]);
  assert.equal(missing.state, "install");
  assert.equal(missing.command, "brew install mole");
  assert.match(missing.detail, /Storage analysis already works/);

  const old = moleSetup([unsupported("mole", {}, ">=1.48, <2.0")]);
  assert.equal(old.state, "upgrade");
  assert.equal(old.command, "brew upgrade mole");

  const ready = moleSetup(mole({ cleanupCategories: true, dryRun: true }));
  assert.equal(ready.state, "ready");
  assert.equal(ready.command, null);
});

test("cleanup review needs a usable Mole with both flags", () => {
  assert.match(cleanupAvailability([]).reason, /Install a supported Mole release/);
  assert.match(
    cleanupAvailability(mole({ cleanupCategories: true, dryRun: true }, false)).reason,
    /Install a supported Mole release/,
    "an unusable Mole is not a Mole",
  );

  const noDryRun = cleanupAvailability(mole({ cleanupCategories: true }));
  assert.equal(noDryRun.available, false);
  assert.match(noDryRun.reason, /does not expose the capabilities/);

  const noCategories = cleanupAvailability(mole({ dryRun: true }));
  assert.equal(noCategories.available, false);

  const ready = cleanupAvailability(mole({ cleanupCategories: true, dryRun: true }));
  assert.equal(ready.available, true);
  assert.equal(ready.reason, "");
});

/**
 * The capability split reaching the screen. Listing applications, previewing a
 * removal, and performing one are three claims, and the page says which it has.
 * See ADR 0027.
 */
test("uninstall is offered in the window only when the backend can preview and perform it", () => {
  assert.equal(
    uninstallOffer(mole({ appInventory: true, uninstallApps: true, dryRun: true })),
    "app",
  );
  assert.equal(uninstallOffer(mole({ appInventory: true })), "terminal");
  // The preview is the whole basis on which the removal is approved, so a
  // backend that could remove without previewing gets the handoff, not a button.
  assert.equal(
    uninstallOffer(mole({ appInventory: true, uninstallApps: true })),
    "terminal",
    "no dry run, no plan to approve",
  );
  assert.equal(
    uninstallOffer(mole({ uninstallApps: true, dryRun: true })),
    "none",
    "no inventory, no page",
  );
  assert.equal(uninstallOffer(mole({ appInventory: true }, false)), "none");
  assert.equal(uninstallOffer([]), "none");
  assert.equal(uninstallOffer(null), "none");
});
