/**
 * Where the window is, resolved from the URL hash.
 *
 * One destination, not three and not seven. The window used to carry a nav item
 * per command surface — Overview, Space Explorer, Developer, Applications,
 * System Status — which described how the backends are arranged rather than
 * what a user came to do. ADR 0026 folded those into `storage`; ADR 0031
 * removed the rail entirely, so what is left is a root and the screens drilled
 * into from it.
 *
 * `storage` with no view **is** that root: the dashboard. A view names the tree
 * browser underneath it, which is why `view` is nullable rather than defaulted
 * — a default view would make the browser the root again and there would be no
 * dashboard to come back to. Old hashes still name the content that absorbed
 * them rather than 404ing into a default.
 */

export type Route = "storage" | "clean" | "activity" | "help";

/** Which slice of the current scan tree the browser is showing. */
export type StorageView = "folders" | "developer" | "applications";

export interface Location {
  route: Route;
  /**
   * `null` is the dashboard. A view is the tree browser, and is carried on
   * every location rather than only on `storage`: it is where the user was
   * looking, so leaving for Clean and coming back should not reset it.
   */
  view: StorageView | null;
  /**
   * The application being inspected, by node id, or `null` for none.
   *
   * Ids belong to the scan that issued them — see `state.rs` — so a link
   * carrying one is only meaningful while that scan is loaded. The Inspector
   * checks, and falls back to the dashboard rather than opening whatever now
   * sits at that index. Kept in the location anyway so that back leaves the
   * Inspector rather than the window.
   */
  inspect: number | null;
}

/** The dashboard, which is the window's one destination. */
export const DEFAULT_LOCATION: Location = { route: "storage", view: null, inspect: null };

export const STORAGE_VIEWS: readonly StorageView[] = ["folders", "developer", "applications"];

/**
 * Hashes from 0.1.x and from anything a user bookmarked. Each one names the page
 * that absorbed it, so an old link lands on the same content rather than on the
 * default.
 */
const RETIRED: Record<string, Location> = {
  // The Overview page's content is the dashboard now, so it lands there rather
  // than in the browser that absorbed it under ADR 0026.
  overview: { route: "storage", view: null, inspect: null },
  space: { route: "storage", view: "folders", inspect: null },
  status: { route: "storage", view: null, inspect: null },
  developer: { route: "storage", view: "developer", inspect: null },
  applications: { route: "storage", view: "applications", inspect: null },
};

/** The segment that introduces an inspected application: `#/storage/app/12`. */
const INSPECT_SEGMENT = "app";

const ROUTES: readonly Route[] = ["storage", "clean", "activity", "help"];

function isRoute(value: string): value is Route {
  return (ROUTES as readonly string[]).includes(value);
}

function isView(value: string): value is StorageView {
  return (STORAGE_VIEWS as readonly string[]).includes(value);
}

/**
 * Onboarding is not a `Route`: it replaces the shell rather than rendering
 * inside it, so it cannot be a nav destination without the nav being wrong
 * while it shows.
 */
export function locationFromHash(hash: string): Location | "onboarding" {
  const [first = "", second = "", third = ""] = hash
    .replace(/^#\/?/, "")
    .split("/")
    .filter(Boolean);
  if (first === "onboarding") return "onboarding";
  const retired = RETIRED[first];
  if (retired) return retired;
  if (!isRoute(first)) return DEFAULT_LOCATION;
  if (first === "storage" && second === INSPECT_SEGMENT) {
    // Digits only, and at least one. `Number("")` is 0, so a bare
    // `#/storage/app` would otherwise open the Inspector on node zero — the
    // scan root — rather than naming no application at all.
    return /^\d+$/.test(third)
      ? { route: "storage", view: null, inspect: Number(third) }
      : DEFAULT_LOCATION;
  }
  return { route: first, view: isView(second) ? second : null, inspect: null };
}

/**
 * Where the window opens.
 *
 * Onboarding existed and was unreachable: the page was written, the hash
 * resolved to it, and nothing ever set that hash — so a first run went straight
 * to a screen that assumes a backend has already been found. A link is honoured
 * whatever this returns; the first-run case only applies when there is no link
 * to honour.
 *
 * `onboarded` is a UI fact rather than a backend one — it records that a person
 * has seen four screens — so it is stored beside the theme rather than in the
 * settings file Rust owns.
 */
export function firstLocation(hash: string, onboarded: boolean): Location | "onboarding" {
  const named = locationFromHash(hash);
  if (named === "onboarding") return "onboarding";
  // A hash means the user arrived somewhere deliberately, and interrupting that
  // with a wizard would lose where they were going.
  if (!onboarded && !hash) return "onboarding";
  return named;
}

/**
 * `#/storage` is the dashboard and `#/storage/folders` is the browser, so the
 * view appears in the hash exactly when there is one. They are different places
 * now, which is why the suffix is no longer suppressed as a default.
 */
export function hashForLocation(location: Location): string {
  if (location.route === "storage" && location.inspect !== null) {
    return `#/storage/${INSPECT_SEGMENT}/${location.inspect}`;
  }
  const suffix = location.route === "storage" && location.view ? `/${location.view}` : "";
  return `#/${location.route}${suffix}`;
}
