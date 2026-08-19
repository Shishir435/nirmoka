import type { CategoryBreakdown, CategoryConsumer, StorageCategory } from "@nirmoka/transport";

/**
 * The dashboard's arithmetic, apart from its rendering.
 *
 * Two decisions live here that are easy to get wrong and impossible to see in a
 * screenshot: which slices the volume bar is made of, and which entries are the
 * biggest things on the disk once an application's real footprint is known.
 */

export interface UsageSlice {
  key: string;
  label: string;
  bytes: number;
  color: string;
}

export type ConsumerMeasure = "size" | "bundle" | "footprint";

export interface RankedConsumer {
  consumer: CategoryConsumer;
  category: StorageCategory;
  measure: ConsumerMeasure;
  icon: string | null;
}

/** A `.app` is an application; everything else is measured as what it is. */
export function isBundle(consumer: CategoryConsumer): boolean {
  return consumer.name.toLowerCase().endsWith(".app");
}

/**
 * The slices of the volume bar, in the order they are drawn.
 *
 * Free space is a slice because the bar is about the volume, not about the
 * scan: a chart of used space alone cannot answer "how much room is left".
 * Where capacity could not be read there is no free slice and the bar is the
 * scan alone, which is still true about what was measured.
 *
 * A scan of one directory leaves a third quantity — space that is in use and
 * was not looked at. It gets its own slice rather than being left as bare
 * track, because bare track is the same colour as free space and the bar would
 * then show occupied disk as room to spare. The slices always add up to the
 * volume, so nothing shows through.
 */
export function usageSlices(
  breakdown: CategoryBreakdown,
  display: Record<StorageCategory, { label: string; color: string }>,
  freeColor: string,
  unscannedColor: string,
): UsageSlice[] {
  const categories = breakdown.categories.map((category) => ({
    key: category.category as string,
    label: display[category.category].label,
    bytes: category.totalBytes,
    color: display[category.category].color,
  }));

  const volume = breakdown.volume;
  if (!volume) return categories;

  const unscanned = Math.max(0, volume.usedBytes - breakdown.scannedBytes);
  return [
    ...categories,
    ...(unscanned > 0
      ? [{ key: "unscanned", label: "Not scanned", bytes: unscanned, color: unscannedColor }]
      : []),
    { key: "free", label: "Free", bytes: volume.freeBytes, color: freeColor },
  ];
}

/** What the bar's widths are a fraction of. */
export function barTotal(breakdown: CategoryBreakdown): number {
  return breakdown.volume ? breakdown.volume.totalBytes : breakdown.scannedBytes;
}

/**
 * Every entry the breakdown reported, largest first.
 *
 * Deliberately not truncated. An application's bundle is a fraction of its
 * footprint — Docker's is under 2 GB against a footprint of tens — so trimming
 * to the visible rows before the footprints arrive would drop the very
 * applications that belong at the top. Rust has already capped each category's
 * consumers, so this list is short; the caller trims once the numbers are real.
 *
 * Every category contributes, so the list mixes applications and directories
 * the way the disk does. Ties break on the path, because two entries of the
 * same size must not swap places between renders.
 */
export function rankConsumers(breakdown: CategoryBreakdown | null): RankedConsumer[] {
  if (!breakdown) return [];
  return breakdown.categories
    .flatMap((category) =>
      category.consumers.map(
        (consumer): RankedConsumer => ({
          consumer,
          category: category.category,
          measure: isBundle(consumer) ? "bundle" : "size",
          icon: null,
        }),
      ),
    )
    .sort(byLargest);
}

/**
 * Where opening this row should take the browser.
 *
 * A directory opens itself. A file has no contents to list, so it opens the
 * directory holding it — `null` being the scan root, which is how the browser
 * addresses it.
 */
export function openTarget(row: RankedConsumer): number | null {
  return row.consumer.isDir ? row.consumer.id : (row.consumer.parentId ?? null);
}

/**
 * Replace a row's number with the application's real footprint.
 *
 * The list is re-sorted afterwards rather than patched in place: a footprint
 * can be twenty times the bundle it replaced, and a list that kept its original
 * order would leave the largest thing on the disk sitting in fourth place.
 */
export function applyFootprint(
  rows: RankedConsumer[],
  id: number,
  totalBytes: number,
): RankedConsumer[] {
  return rows
    .map((row) =>
      row.consumer.id === id
        ? {
            ...row,
            consumer: { ...row.consumer, totalBytes },
            measure: "footprint" as const,
          }
        : row,
    )
    .sort(byLargest);
}

/** Icons never reorder anything: they are decoration. */
export function applyIcon(
  rows: RankedConsumer[],
  id: number,
  icon: string | null,
): RankedConsumer[] {
  return rows.map((row) => (row.consumer.id === id ? { ...row, icon } : row));
}

function byLargest(left: RankedConsumer, right: RankedConsumer): number {
  return (
    right.consumer.totalBytes - left.consumer.totalBytes ||
    left.consumer.path.localeCompare(right.consumer.path)
  );
}
