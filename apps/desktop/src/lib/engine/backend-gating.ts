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

export interface BackendSetup {
  state: "checking" | "ready" | "install" | "upgrade" | "unavailable";
  title: string;
  detail: string;
  command: string | null;
}

const usable = (backends: Backend[] | null, id: string) =>
  backends?.find((backend) => backend.id === id && backend.usable) ?? null;

/**
 * The recovery action for the baseline scanner.
 *
 * Installation and upgrade are different instructions. Folding an unsupported
 * version into "missing" sends a person to install something already present,
 * and was the source of the old onboarding's misleading combined command.
 */
export function scannerSetup(
  backends: Backend[] | null,
  selection: BackendSelection | null,
): BackendSetup {
  if (backends === null) {
    return {
      state: "checking",
      title: "Checking the scanner",
      detail: "Looking for a supported disk scanner on this Mac.",
      command: null,
    };
  }

  if (selection === null) {
    return {
      state: "unavailable",
      title: "Scanner check did not finish",
      detail: "Nirmoka could not resolve which installed backend should scan. Check again.",
      command: null,
    };
  }

  if (selection.scanner) {
    const scanner = backends.find((backend) => backend.id === selection.scanner);
    return {
      state: "ready",
      title: `${scanner?.displayName ?? selection.scanner} is ready`,
      detail: "Nirmoka can start with a read-only storage scan.",
      command: null,
    };
  }

  const unsupported = backends.find(
    (backend) => backend.capabilities.scan && backend.detection?.state === "unsupportedVersion",
  );
  if (unsupported?.detection?.state === "unsupportedVersion") {
    return {
      state: "upgrade",
      title: `Update ${unsupported.displayName}`,
      detail: `Version ${unsupported.detection.version} is installed, but this Nirmoka build understands ${unsupported.detection.supported}.`,
      command: "brew upgrade ncdu",
    };
  }

  return {
    state: "install",
    title: "Install the disk scanner",
    detail:
      "The Homebrew installation normally includes ncdu. Install it separately if it is missing.",
    command: "brew install ncdu",
  };
}

/** Contextual setup for Mole, which enhances Nirmoka but never blocks scanning. */
export function moleSetup(backends: Backend[] | null): BackendSetup {
  if (backends === null) {
    return {
      state: "checking",
      title: "Checking Mole",
      detail: "Looking for optional cleanup and complete uninstall capabilities.",
      command: null,
    };
  }

  const mole = backends.find((backend) => backend.id === "mole");
  if (mole?.usable) {
    return {
      state: "ready",
      title: "Mole is ready",
      detail: "Curated cleanup and complete application uninstall are available.",
      command: null,
    };
  }

  if (mole?.detection?.state === "unsupportedVersion") {
    return {
      state: "upgrade",
      title: "Update Mole",
      detail: `Mole ${mole.detection.version} is installed, but this Nirmoka build understands ${mole.detection.supported}.`,
      command: "brew upgrade mole",
    };
  }

  if (mole?.error) {
    return {
      state: "unavailable",
      title: "Mole could not be checked",
      detail: mole.error,
      command: null,
    };
  }

  return {
    state: "install",
    title: "Add complete cleanup with Mole",
    detail:
      "Storage analysis already works. Mole adds curated cleanup and removes applications together with their associated data.",
    command: "brew install mole",
  };
}

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
 * Three states, and the middle one is not a placeholder. `app` means the backend
 * can both preview and perform the removal. `terminal` is the honest fallback for
 * a backend that lists applications and the exact name its command takes but
 * cannot be driven to remove one — the page then names the command instead of
 * offering a button that dies. `none` means there is no inventory at all.
 *
 * Both flags are required for `app`: a preview is the whole basis on which the
 * removal is approved, so a release that could remove without previewing would
 * get the Terminal handoff, not the button. See ADR 0027.
 */
export function uninstallOffer(backends: Backend[] | null): "app" | "terminal" | "none" {
  const mole = usable(backends, "mole");
  if (!mole?.capabilities.appInventory) return "none";
  return mole.capabilities.uninstallApps && mole.capabilities.dryRun ? "app" : "terminal";
}
