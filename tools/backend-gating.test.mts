import assert from "node:assert/strict";
import test from "node:test";

import {
  cleanupAvailability,
  scanAvailability,
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
 * The capability split from ADR 0021 reaching the screen: listing applications
 * is offered, removing one is not, and the page says which.
 */
test("uninstall is offered in Terminal while the backend only lists apps", () => {
  assert.equal(uninstallOffer(mole({ appInventory: true })), "terminal");
  assert.equal(uninstallOffer(mole({ appInventory: true, uninstallApps: true })), "app");
  assert.equal(uninstallOffer(mole({ uninstallApps: true })), "none", "no inventory, no page");
  assert.equal(uninstallOffer(mole({ appInventory: true }, false)), "none");
  assert.equal(uninstallOffer([]), "none");
  assert.equal(uninstallOffer(null), "none");
});
