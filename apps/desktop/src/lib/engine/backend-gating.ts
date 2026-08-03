/**
 * What the window may offer, given what is installed.
 *
 * Every one of these answers used to be an inline condition on a page, which is
 * where "degrade, don't lie" quietly breaks: a control gated on the wrong flag
 * either hides an ability the backend has or offers one it does not. Collected
 * here so the reasons are testable and the pages state them the same way twice.
 *
 * Only type imports, so the module runs under `node --test` with no DOM.
 */

import type { Backend, BackendSelection } from "@nirmoka/transport";

export interface Availability {
  available: boolean;
  /** Why not, in the window's own words. Empty when it is available. */
  reason: string;
}

const usable = (backends: Backend[] | null, id: string) =>
  backends?.find((backend) => backend.id === id && backend.usable) ?? null;

/**
 * Whether a scan can be started, and what to say when it cannot.
 *
 * `null` backends means detection has not answered yet, which is not the same as
 * "nothing is installed" and must not be reported as such.
 */
export function scanAvailability(
  backends: Backend[] | null,
  selection: BackendSelection | null,
): Availability {
  if (backends === null) return { available: false, reason: "Detecting backends…" };
  if (selection?.scanner) return { available: true, reason: "" };

  return {
    available: false,
    reason: "No supported scanner is installed. Install ncdu 2.x, then refresh backend detection.",
  };
}

/** Whether Mole can publish a cleanup preview, and what to say when it cannot. */
export function cleanupAvailability(backends: Backend[] | null): Availability {
  const mole = usable(backends, "mole");
  if (!mole) {
    return {
      available: false,
      reason:
        "Install a supported Mole release to use its curated cleanup rules. ncdu scans disks; it does not decide what is safe to clean.",
    };
  }
  // Both flags, because a preview is a dry run of a category cleanup: a release
  // with one and not the other cannot produce the plan this page reviews.
  if (!mole.capabilities.cleanupCategories || !mole.capabilities.dryRun) {
    return {
      available: false,
      reason: "This Mole version does not expose the capabilities required for a cleanup preview.",
    };
  }
  return { available: true, reason: "" };
}

/**
 * What the Applications page may say about removing an application.
 *
 * `terminal` is the honest middle state, and the one that holds today: Mole
 * lists applications and the exact name its command takes, but every named
 * uninstall stops at a confirmation prompt with no non-interactive flag, and
 * answering another tool's safety prompt is not something this app does. See
 * ADR 0021.
 */
export function uninstallOffer(backends: Backend[] | null): "app" | "terminal" | "none" {
  const mole = usable(backends, "mole");
  if (!mole?.capabilities.appInventory) return "none";
  return mole.capabilities.uninstallApps ? "app" : "terminal";
}
