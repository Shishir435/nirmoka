import type {
  AppFootprint,
  CleanupItem,
  CleanupPreview,
  StorageComponent,
} from "@nirmoka/transport";

/**
 * What the Inspector shows about one application, apart from how it looks.
 *
 * Two things live here because both are easy to get subtly wrong and neither is
 * visible in a screenshot: which of Mole's cleanup rows belong to this
 * application, and what share of its footprint each component accounts for.
 */

/** One of Mole's cleanup rows, attributed to the application being inspected. */
export interface Reclaimable {
  /** Mole's own category name — never reworded. See ADR 0030. */
  category: string;
  item: CleanupItem;
}

/**
 * A path as Mole and as the footprint write it, reduced to one form.
 *
 * Mole abbreviates the home directory and the footprint does not, so the two
 * describe the same directory in different words. Comparing them raw finds
 * nothing at all.
 */
function normalise(path: string, home: string): string {
  const expanded = path.startsWith("~/") && home ? `${home}${path.slice(1)}` : path;
  // A trailing separator or a wildcard is Mole describing a directory's
  // contents; the directory is what matches.
  return expanded.replace(/[/*]+$/, "").toLowerCase();
}

/**
 * Mole's cleanup rows that fall inside this application's footprint.
 *
 * Attribution is a path-prefix comparison against paths the footprint already
 * established — arithmetic on published data, not a judgement about what is
 * safe to remove. Nirmoka contributes no rules and no rows: everything here was
 * selected by Mole, and is shown under Mole's own category name.
 *
 * @see docs/adr/0030-safety-language-comes-from-the-backend-or-not-at-all.md
 */
export function reclaimableFor(
  footprint: AppFootprint,
  preview: CleanupPreview | null,
  home: string,
): Reclaimable[] {
  if (!preview) return [];

  // Only the components attributed by identifier. A vendor-named guess is not
  // firm enough to hang a removal off — ADR 0028.
  const owned = footprint.components
    .filter((component) => component.certain)
    .flatMap((component) => component.paths.map((path) => normalise(path.path, home)))
    .filter(Boolean);
  if (owned.length === 0) return [];

  const found: Reclaimable[] = [];
  for (const category of preview.categories) {
    for (const item of category.items) {
      const path = normalise(item.path, home);
      const inside = owned.some((base) => path === base || path.startsWith(`${base}/`));
      if (inside) found.push({ category: category.name, item });
    }
  }
  return found;
}

/**
 * A component's share of the footprint, for the bar beside it.
 *
 * Of the total the components add up to, not of the disk: the Inspector is
 * about how one application's own storage divides, and a percentage of the
 * volume would be a rounding error for every row.
 */
export function componentShare(component: StorageComponent, footprint: AppFootprint): number {
  // The uncertain component is excluded from the total — see ADR 0028 — so
  // measuring it against that total could exceed 1. It is shown beside the
  // others rather than as a share of them.
  if (!component.certain || footprint.totalBytes === 0) return 0;
  return Math.min(1, component.totalBytes / footprint.totalBytes);
}

/** Whether any part of this footprint is a lower bound rather than a total. */
export function isLowerBound(footprint: AppFootprint): boolean {
  return (
    footprint.unmeasuredPaths > 0 || footprint.components.some((component) => !component.complete)
  );
}
