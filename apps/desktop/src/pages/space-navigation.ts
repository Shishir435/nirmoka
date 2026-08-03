/**
 * Where the browser is, and how it got there.
 *
 * History lives here as data rather than in component state so that back and
 * forward are testable without a DOM: the whole model is one array and one
 * index, and every keystroke in the tree view resolves to a function in this
 * file.
 *
 * Every entry names its scan. Node ids are indices into a per-scan arena, so an
 * entry kept across a rescan would resolve against the new tree and open a
 * different directory — see `crates/app/src/state.rs`.
 */

export interface SpaceLocation {
  scanId: number;
  parentId: number | null;
}

export function parentIdForScan(location: SpaceLocation | null, scanId: number | null) {
  return location?.scanId === scanId ? location.parentId : null;
}

/**
 * Visited locations, oldest first, with a cursor.
 *
 * `index` is where the user is now; anything after it is forward history, which
 * a new navigation discards.
 */
export interface SpaceHistory {
  entries: SpaceLocation[];
  index: number;
}

export const EMPTY_HISTORY: SpaceHistory = { entries: [], index: -1 };

export function currentLocation(history: SpaceHistory): SpaceLocation | null {
  return history.entries[history.index] ?? null;
}

/**
 * Record a navigation.
 *
 * Reopening the directory already on screen changes nothing — otherwise a
 * double press of Enter would leave two identical entries and "back" would
 * appear to do nothing. A location from a different scan starts a fresh history
 * rather than extending one whose ids no longer mean anything.
 */
export function visit(history: SpaceHistory, location: SpaceLocation): SpaceHistory {
  const here = currentLocation(history);
  if (here && here.scanId !== location.scanId) {
    return { entries: [location], index: 0 };
  }
  if (here && here.parentId === location.parentId) return history;

  const entries = [...history.entries.slice(0, history.index + 1), location];
  return { entries, index: entries.length - 1 };
}

export function canGoBack(history: SpaceHistory) {
  return history.index > 0;
}

export function canGoForward(history: SpaceHistory) {
  return history.index >= 0 && history.index < history.entries.length - 1;
}

export function goBack(history: SpaceHistory): SpaceHistory {
  return canGoBack(history) ? { ...history, index: history.index - 1 } : history;
}

export function goForward(history: SpaceHistory): SpaceHistory {
  return canGoForward(history) ? { ...history, index: history.index + 1 } : history;
}
