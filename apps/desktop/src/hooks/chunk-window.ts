/**
 * Which chunks a visible range needs.
 *
 * Split out of `use-directory` so the arithmetic is testable at the sizes that
 * matter. A directory of a quarter of a million entries is the case invariant 5
 * exists for, and "does scrolling to row 249,000 ask for one chunk or 2,490"
 * is a question a test should answer rather than a profiler.
 */

/**
 * Rows per request. Big enough that a scroll of one screen is usually already
 * loaded, small enough that a directory of a hundred thousand entries never
 * arrives in one message. The Rust side caps anything larger regardless.
 */
export const CHUNK = 100;

/**
 * Chunk offsets covering `[start, end]`, clamped to a directory of `total` rows.
 *
 * Returns offsets, not indices, because that is what a request takes and what
 * the in-flight set is keyed by. An empty array means the range asks for
 * nothing — a range past the end, or a directory with no rows.
 */
export function chunkOffsets(start: number, end: number, total: number): number[] {
  if (total <= 0) return [];

  const from = Math.max(0, Math.min(start, total - 1));
  const to = Math.max(0, Math.min(end, total - 1));
  if (to < from) return [];

  const first = Math.floor(from / CHUNK);
  const last = Math.floor(to / CHUNK);

  const offsets: number[] = [];
  for (let chunk = first; chunk <= last; chunk += 1) offsets.push(chunk * CHUNK);
  return offsets;
}
