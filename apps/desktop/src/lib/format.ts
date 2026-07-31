/**
 * Display formatting.
 *
 * This mirrors `nirmoka_core::format_bytes`, deliberately. Sizes are the number
 * the whole app is about, and shipping them pre-rendered from Rust would put a
 * presentation decision — units, precision, locale — behind an IPC call, where
 * the UI cannot change it without a Rust build. The rule stays the same on both
 * sides: binary units, one decimal below 10.
 */

const UNITS = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"] as const;

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;

  let value = bytes;
  let unit = 0;

  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }

  return `${value.toFixed(value < 10 ? 1 : 0)} ${UNITS[unit]}`;
}

export function formatCount(count: number): string {
  return count.toLocaleString();
}

/** "1 entry" / "2 entries" — a count of one reading as plural looks like a bug. */
export function plural(count: number, one: string, many: string): string {
  return `${formatCount(count)} ${count === 1 ? one : many}`;
}
