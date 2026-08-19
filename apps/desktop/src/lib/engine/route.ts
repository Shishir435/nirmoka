/**
 * Where the window is, resolved from the URL hash.
 *
 * Three destinations, not seven. The window used to carry a nav item per
 * command surface — Overview, Space Explorer, Developer, Applications, System
 * Status — which described how the backends are arranged rather than what a
 * user came to do. Everything derived from one scan tree now lives on `storage`
 * as a view of it, and the old hashes redirect there rather than 404ing into a
 * default. See ADR 0026.
 */

export type Route = "storage" | "clean" | "activity" | "help";

/** Which slice of the current scan tree `storage` is showing. */
export type StorageView = "folders" | "developer" | "applications";

export interface Location {
  route: Route;
  /**
   * Carried on every location, not only on `storage`. It is where the user was
   * looking, so leaving for Clean and coming back should not silently reset it.
   */
  view: StorageView;
}

export const DEFAULT_LOCATION: Location = { route: "storage", view: "folders" };

export const STORAGE_VIEWS: readonly StorageView[] = ["folders", "developer", "applications"];

/**
 * Hashes from 0.1.x and from anything a user bookmarked. Each one names the page
 * that absorbed it, so an old link lands on the same content rather than on the
 * default.
 */
const RETIRED: Record<string, Location> = {
  overview: { route: "storage", view: "folders" },
  space: { route: "storage", view: "folders" },
  status: { route: "storage", view: "folders" },
  developer: { route: "storage", view: "developer" },
  applications: { route: "storage", view: "applications" },
};

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
  const [first = "", second = ""] = hash.replace(/^#\/?/, "").split("/").filter(Boolean);
  if (first === "onboarding") return "onboarding";
  const retired = RETIRED[first];
  if (retired) return retired;
  if (!isRoute(first)) return DEFAULT_LOCATION;
  return { route: first, view: isView(second) ? second : DEFAULT_LOCATION.view };
}

/**
 * The default view is left out of the hash. `#/storage` and `#/storage/folders`
 * are the same place, and only one of them should appear in a shared link.
 */
export function hashForLocation(location: Location): string {
  const suffix =
    location.route === "storage" && location.view !== DEFAULT_LOCATION.view
      ? `/${location.view}`
      : "";
  return `#/${location.route}${suffix}`;
}
