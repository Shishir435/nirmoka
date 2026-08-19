import type {
  CategoryBreakdown,
  CategoryConsumer,
  StorageCategory,
  VolumeInfo,
} from "@nirmoka/transport";

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
 * The volume the bar is drawn against, or `null` to draw the scan alone.
 *
 * A scan is not guaranteed to live on one filesystem. Scanning `/` walks into
 * `/Volumes`, and a mounted disk or network share under home does the same, so
 * the scanned bytes can exceed everything this filesystem holds. Framing that
 * scan against this volume's capacity would draw category slices, free space,
 * and a clamped-to-zero unscanned slice that together add up to more than the
 * bar: the widths stop being shares of anything, and the last slices are
 * clipped off the end.
 *
 * Where the two numbers cannot both be true, they are not compared at all. The
 * bar divides the scan regardless — see [`usageSlices`] — so what this now
 * guards is the caption and the free-space card, which would otherwise report a
 * negative remainder as though part of the disk had gone missing.
 */
export function barVolume(breakdown: CategoryBreakdown): VolumeInfo | null {
  const volume = breakdown.volume;
  if (!volume) return null;
  return breakdown.scannedBytes <= volume.usedBytes ? volume : null;
}

/**
 * The slices of the bar, in the order they are drawn.
 *
 * The bar divides **what was scanned**, not the volume. Framing it against
 * capacity was tried and is worse everywhere the two differ: a scan of
 * `~/Downloads` on a 253 GiB disk drew every category as a hairline beside
 * 233 GiB of grey, so the one chart on the screen said nothing about what the
 * user had just asked for.
 *
 * Capacity is a real number and it is still shown — as Used and Free in the
 * header, as its own card in the grid, and as a sentence naming how much of the
 * disk this scan covered. It is simply not the scale, because the categories
 * are not shares of it. On a whole-disk scan the two coincide, which is the
 * case the design draws.
 *
 * This also retires the free and not-scanned slices. Both existed to make a
 * volume-framed bar add up, and [`barVolume`]'s cross-filesystem guard exists
 * for the same reason — it now guards the caption instead, which is the only
 * place the two numbers still meet.
 */
export function usageSlices(
  breakdown: CategoryBreakdown,
  display: Record<StorageCategory, { label: string; color: string }>,
): UsageSlice[] {
  return breakdown.categories.map((category) => ({
    key: category.category as string,
    label: display[category.category].label,
    bytes: category.totalBytes,
    color: display[category.category].color,
  }));
}

/** What the bar's widths are a fraction of. */
export function barTotal(breakdown: CategoryBreakdown): number {
  return breakdown.scannedBytes;
}

/**
 * How much of the volume's used space this scan did not look at.
 *
 * Zero where there is nothing to say: capacity unreadable, the whole disk
 * scanned, or a scan that crossed onto another filesystem and so cannot be
 * subtracted from this one — see [`barVolume`].
 */
export function unscannedBytes(breakdown: CategoryBreakdown): number {
  const volume = barVolume(breakdown);
  return volume ? volume.usedBytes - breakdown.scannedBytes : 0;
}

/**
 * Every entry the breakdown reported, largest first.
 *
 * Deliberately not truncated. An application's bundle is a fraction of its
 * footprint — Docker's is under 2 GB against a footprint of tens — so trimming
 * to the visible rows before the footprints arrive would drop the very
 * applications that belong at the top. Rust caps ordinary consumers and exempts
 * application bundles for that reason, so this list is a handful of directories
 * plus the installed applications; the caller trims once the numbers are real.
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
