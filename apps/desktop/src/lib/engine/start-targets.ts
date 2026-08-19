/**
 * What the window offers before a scan exists.
 *
 * The cold start used to be an empty state telling the user to type a path into
 * a text field, which is a terminal idiom rendered as a dashed rectangle. Volume
 * capacity needs no backend and no scan, so the first screen can answer "how
 * full is this disk" and offer the handful of directories a disk tool is
 * actually opened for.
 *
 * Kept pure and separate from the component so the clamping below is covered by
 * `node --test` rather than by dragging a window to the edge of a full disk.
 */

export interface StartTarget {
  /** Stable key, and the accessible name the button is found by. */
  readonly id: string;
  readonly label: string;
  /** Passed to `startScan` verbatim; Rust expands a leading `~`. */
  readonly path: string;
  readonly hint: string;
}

/**
 * Ordered by how often a disk tool is opened for each one, not alphabetically.
 * Home is first because it is the default and the one that finds the surprise;
 * the rest are the directories that turn out to be the answer most of the time.
 */
export const START_TARGETS: readonly StartTarget[] = [
  { id: "home", label: "Home", path: "~", hint: "Everything you own" },
  { id: "downloads", label: "Downloads", path: "~/Downloads", hint: "Usually the quick win" },
  { id: "caches", label: "Caches", path: "~/Library/Caches", hint: "Rebuilt when needed" },
  { id: "applications", label: "Applications", path: "/Applications", hint: "Installed apps" },
];

/**
 * How much of the volume is in use, as a fraction between 0 and 1.
 *
 * `df` does not report `used + free === total` on macOS — a 494 GB disk reports
 * 238 used and 230 free, with the rest reserved or purgeable. So the bar is
 * drawn as used-over-total with the remainder as track, and free space is
 * stated as its own number rather than implied by the gap. Drawing used and
 * free as two adjacent segments would leave a mystery slice.
 */
export function usedFraction(volume: { totalBytes: number; usedBytes: number }): number {
  const { totalBytes, usedBytes } = volume;
  // A volume with no capacity is not 100% full, and dividing by it is worse.
  if (!Number.isFinite(totalBytes) || totalBytes <= 0) return 0;
  if (!Number.isFinite(usedBytes) || usedBytes <= 0) return 0;
  return Math.min(1, usedBytes / totalBytes);
}

/**
 * Whether a volume is full enough to say so.
 *
 * macOS starts warning around 90%, and a disk tool that stays silent while the
 * disk is nearly full has missed the one thing the user came to find out.
 */
export function isNearlyFull(volume: { totalBytes: number; usedBytes: number }): boolean {
  return usedFraction(volume) >= 0.9;
}
