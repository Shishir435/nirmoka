/**
 * Reviewing an application, approving the plan, and removing it — as data.
 *
 * Four steps rather than the Trash flow's three, and the extra one is the point.
 * A path being moved needs a confirmation; an application being uninstalled needs
 * a *plan* first, because "uninstall Chrome" and "delete these 34 paths, two of
 * which I will not touch" are different things to agree to. So the backend
 * produces the plan, this holds it, and only then is there something to confirm.
 *
 * Once the run starts it is not abandoned. The backend is moving files and may be
 * holding up its own authorization dialog; a reducer that forgot about it would
 * leave the window claiming nothing is happening. See ADR 0027.
 *
 * Only type imports, so the module runs under `node --test` with no DOM.
 */

import type {
  UninstallItem,
  UninstallOperation,
  UninstallPreparation,
  UninstallPreview,
} from "@nirmoka/transport";

export interface UninstallState {
  /** The identifier being reviewed or removed, for the row's own spinner. */
  activeName: string | null;
  /** The backend's plan, which the sheet is open for. */
  preview: UninstallPreview | null;
  /** A live confirmation. Present only after the plan has been shown. */
  preparation: UninstallPreparation | null;
  /**
   * Which request is being waited on, or `null`.
   *
   * Identified for the same reason the Trash flow identifies its requests: a
   * preview takes seconds, the user can click another application in the
   * meantime, and a late reply from the abandoned one would otherwise open a
   * sheet describing an application nobody asked about.
   */
  pendingRequestId: number | null;
  reviewing: boolean;
  running: boolean;
  runningRequestId: number | null;
  /** Identifiers removed in this session, so their rows stop offering it. */
  removedNames: string[];
  last: UninstallOperation | null;
  error: string | null;
}

export type UninstallEvent =
  | { type: "reviewStarted"; requestId: number; name: string }
  | { type: "reviewed"; requestId: number; preview: UninstallPreview }
  | { type: "reviewFailed"; requestId: number; message: string }
  /** The plan was read and accepted; Rust issued a token for it. */
  | { type: "prepared"; requestId: number; preparation: UninstallPreparation }
  | { type: "dismissed" }
  | { type: "runStarted"; requestId: number }
  | { type: "removed"; requestId: number; operation: UninstallOperation }
  | { type: "runFailed"; requestId: number; message: string }
  /** The inventory was reloaded, so every plan in hand describes an older disk. */
  | { type: "inventoryReloaded" };

export const INITIAL_UNINSTALL: UninstallState = {
  activeName: null,
  preview: null,
  preparation: null,
  pendingRequestId: null,
  reviewing: false,
  running: false,
  runningRequestId: null,
  removedNames: [],
  last: null,
  error: null,
};

export function reduceUninstall(state: UninstallState, event: UninstallEvent): UninstallState {
  switch (event.type) {
    case "reviewStarted":
      return {
        ...state,
        reviewing: true,
        pendingRequestId: event.requestId,
        activeName: event.name,
        preview: null,
        preparation: null,
        error: null,
        last: null,
      };

    case "reviewed":
      return answering(state, event.requestId)
        ? { ...state, reviewing: false, preview: event.preview }
        : state;

    case "reviewFailed":
      return answering(state, event.requestId)
        ? {
            ...state,
            reviewing: false,
            pendingRequestId: null,
            activeName: null,
            preview: null,
            error: event.message,
          }
        : state;

    // The plan stays on screen beside its confirmation. Replacing it with a
    // summary at the moment of approval would ask the user to agree to something
    // they can no longer read.
    case "prepared":
      return answering(state, event.requestId)
        ? { ...state, preparation: event.preparation }
        : state;

    case "dismissed":
      return {
        ...state,
        preview: null,
        preparation: null,
        pendingRequestId: null,
        activeName: null,
        reviewing: false,
      };

    case "runStarted":
      // The token is spent the moment it is sent, and the plan it described is
      // about to stop being true. Both go, so a second click has nothing to
      // resubmit.
      return {
        ...state,
        preview: null,
        preparation: null,
        running: true,
        runningRequestId: event.requestId,
        error: null,
      };

    case "removed": {
      if (!completing(state, event.requestId)) return state;
      // Marked from what the backend *reported*, not from what was asked. A
      // partial run removed some of them, and marking the rest would claim a
      // removal that did not happen.
      const removedNames = event.operation.removed.length > 0 ? event.operation.removed : [];
      return {
        ...state,
        running: false,
        runningRequestId: null,
        activeName: null,
        last: event.operation,
        removedNames: [
          ...state.removedNames,
          ...removedNames.filter((name) => !state.removedNames.includes(name)),
        ],
      };
    }

    case "runFailed":
      // An `Err` from Rust means the run never started — a spent token, an
      // identifier the backend no longer lists, a version that changed since the
      // review. Nothing was removed, so nothing is marked.
      return completing(state, event.requestId)
        ? {
            ...state,
            running: false,
            runningRequestId: null,
            activeName: null,
            error: event.message,
          }
        : state;

    // A run already underway is not cancelled here: files are moving, and the
    // result still belongs in the journal.
    case "inventoryReloaded":
      return state.running
        ? {
            ...state,
            preview: null,
            preparation: null,
            reviewing: false,
            pendingRequestId: null,
            error: null,
          }
        : { ...INITIAL_UNINSTALL, removedNames: state.removedNames, last: state.last };
  }
}

function answering(state: UninstallState, requestId: number) {
  return state.pendingRequestId === requestId;
}

function completing(state: UninstallState, requestId: number) {
  return state.running && state.runningRequestId === requestId;
}

/** Whether an application can be reviewed for removal right now. */
export function canUninstall(state: UninstallState, name: string | null): name is string {
  return (
    name !== null &&
    !state.reviewing &&
    !state.running &&
    state.preview === null &&
    !state.removedNames.includes(name)
  );
}

/**
 * How many paths the plan says will actually be removed, and how many it says it
 * will leave behind.
 *
 * Two numbers rather than one total, because they are different promises and a
 * combined count would overstate the first.
 */
/**
 * The plan's paths, grouped by the location they sit in.
 *
 * The same vocabulary `attribution.rs` uses for a footprint — Application,
 * Containers, Caches, Application Support, Logs, Preferences — so the screen
 * that says what an application costs and the screen that says what removing it
 * touches name the same places the same way. A flat list of thirty paths is a
 * transcript; these are the parts of the application being removed.
 *
 * Order is fixed rather than by size, so the same application reads the same
 * way twice, and `Other` is last because it is the group with no location.
 */
const PLAN_GROUPS = [
  "Application",
  "Containers",
  "Application Support",
  "Caches",
  "Logs",
  "Preferences",
  "Other",
] as const;

export type PlanGroupLabel = (typeof PLAN_GROUPS)[number];

export interface PlanGroup {
  label: PlanGroupLabel;
  items: UninstallItem[];
}

/** Which group a path belongs to, from where it sits rather than what it is. */
export function groupOf(displayPath: string): PlanGroupLabel {
  const path = displayPath.toLowerCase();
  if (path.startsWith("/applications/") || path.endsWith(".app")) return "Application";
  if (path.includes("/library/containers/") || path.includes("/library/application scripts/")) {
    return "Containers";
  }
  if (path.includes("/library/application support/")) return "Application Support";
  if (
    path.includes("/library/caches/") ||
    path.includes("/library/httpstorages/") ||
    path.includes("/library/webkit/")
  ) {
    return "Caches";
  }
  if (path.includes("/library/logs/")) return "Logs";
  if (
    path.includes("/library/preferences/") ||
    path.includes("/library/saved application state/")
  ) {
    return "Preferences";
  }
  return "Other";
}

/** Group a plan's paths, dropping groups nothing fell into. */
export function groupPlan(items: UninstallItem[]): PlanGroup[] {
  return PLAN_GROUPS.map((label) => ({
    label,
    items: items.filter((item) => groupOf(item.displayPath) === label),
  })).filter((group) => group.items.length > 0);
}

/**
 * What the backend says it will leave behind.
 *
 * Shown where the approved design puts a "keep user data" choice, because no
 * flag backs that choice and this is the true answer to the question it asks —
 * see ADR 0029. Quoted from the dry run rather than described.
 */
export function survivingItems(items: UninstallItem[]): UninstallItem[] {
  return items.filter((item) => item.scope !== "removed");
}

export function planCounts(preview: UninstallPreview) {
  const items = preview.apps.flatMap((app) => app.items);
  return {
    removed: items.filter((item) => item.scope !== "reviewOnly").length,
    reviewOnly: items.filter((item) => item.scope === "reviewOnly").length,
  };
}

/**
 * What to say after a run.
 *
 * Reports the backend's own completion rather than flattening everything into
 * success or failure. A partial run removed something, and saying it failed would
 * send someone looking for an application that is already gone.
 */
export function uninstallOutcomeMessage(operation: UninstallOperation) {
  const freed =
    operation.reportedFreed === null ? "" : `, freeing about ${operation.reportedFreed}`;
  const names = operation.removed.length > 0 ? operation.removed.join(", ") : "nothing";

  let message: string;
  switch (operation.completion) {
    case "finished":
      message = `${names} moved to the Trash${freed}. Recover it there with Put Back.`;
      break;
    case "partial":
      message = `${names} moved to the Trash${freed}. ${operation.backend} could not remove: ${operation.failed.join("; ")}.`;
      break;
    case "cancelled":
      message = `The uninstall was stopped part way through. Anything already moved is in the Trash.`;
      break;
    case "failed":
      message = `${operation.backend} did not finish the uninstall. Check the Trash before trying again.`;
      break;
  }

  return operation.logError === null
    ? message
    : `${message} It could not be added to the operation log: ${operation.logError}`;
}
