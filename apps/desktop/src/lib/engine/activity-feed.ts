/**
 * One timeline over three journals.
 *
 * Rust keeps trashed items, cleanup runs, and deletions in one append-only log
 * and one id space, and hands them back as three typed lists. The window used to
 * show only the deletions, so a file the user had just moved to the Trash
 * appeared in no history at all. Merging here rather than in Rust keeps the
 * commands returning exactly what they record — see ADR 0026.
 */

import type { CleanupOperation, DeleteOperation, TrashOperation } from "@nirmoka/transport";

export type ActivityEntry =
  | { kind: "trashed"; id: number; atMs: number; operation: TrashOperation }
  | { kind: "cleaned"; id: number; atMs: number; operation: CleanupOperation }
  | { kind: "deleted"; id: number; atMs: number; operation: DeleteOperation };

export interface Journals {
  trashed: TrashOperation[];
  cleaned: CleanupOperation[];
  deleted: DeleteOperation[];
}

export const NO_JOURNALS: Journals = { trashed: [], cleaned: [], deleted: [] };

/**
 * Newest first. Ties break on the id, descending, and that is exact rather than
 * arbitrary: the three journals share one counter, so a larger id happened
 * later even when two events land in the same millisecond.
 */
export function mergeActivity(journals: Journals): ActivityEntry[] {
  const entries: ActivityEntry[] = [
    ...journals.trashed.map(
      (operation): ActivityEntry => ({
        kind: "trashed",
        id: operation.id,
        atMs: operation.trashedAtMs,
        operation,
      }),
    ),
    ...journals.cleaned.map(
      (operation): ActivityEntry => ({
        kind: "cleaned",
        id: operation.id,
        atMs: operation.executedAtMs,
        operation,
      }),
    ),
    ...journals.deleted.map(
      (operation): ActivityEntry => ({
        kind: "deleted",
        id: operation.id,
        atMs: operation.deletedAtMs,
        operation,
      }),
    ),
  ];
  return entries.sort((left, right) => right.atMs - left.atMs || right.id - left.id);
}

/**
 * What an entry's recovery actually is. Three different answers, and none of
 * them is a button in this window: the Trash is restored by the Finder, a
 * cleanup run has no per-path receipt to restore from, and only a recorded
 * recoverable deletion can be undone through a backend.
 */
export type Recovery = "putBack" | "none" | "undoable" | "undone";

export function recoveryOf(entry: ActivityEntry): Recovery {
  switch (entry.kind) {
    case "trashed":
      return "putBack";
    case "cleaned":
      return "none";
    case "deleted":
      return entry.operation.undone ? "undone" : entry.operation.recoverable ? "undoable" : "none";
  }
}

/** Whether an event happened but could not be written to the journal. */
export function logErrorOf(entry: ActivityEntry): string | null {
  return entry.operation.logError;
}

export function activityCounts(entries: ActivityEntry[]) {
  return {
    total: entries.length,
    trashed: entries.filter((entry) => entry.kind === "trashed").length,
    cleaned: entries.filter((entry) => entry.kind === "cleaned").length,
    deleted: entries.filter((entry) => entry.kind === "deleted").length,
  };
}

/**
 * Bytes this window can defend. Only trashed items carry a measured size: a
 * cleanup run reports Mole's rounded review text and re-discovers candidates as
 * it goes, and existing deletion receipts record no size at all. Summing those
 * would be arithmetic on labels.
 */
export function measuredBytes(entries: ActivityEntry[]): number {
  return entries.reduce(
    (total, entry) => (entry.kind === "trashed" ? total + entry.operation.totalBytes : total),
    0,
  );
}
