import assert from "node:assert/strict";
import test from "node:test";

import {
  activityCounts,
  measuredBytes,
  mergeActivity,
  NO_JOURNALS,
  recoveryOf,
  type ActivityEntry,
} from "../apps/desktop/src/lib/engine/activity-feed.ts";

const trashed = (id: number, atMs: number, totalBytes = 1024) =>
  ({
    id,
    targetPath: `/Users/person/Downloads/old-${id}`,
    totalBytes,
    trashedAtMs: atMs,
    logError: null,
  }) as never;

const cleaned = (id: number, atMs: number) =>
  ({
    id,
    backend: "mole",
    backendVersion: "1.48.1",
    previewGeneratedAt: "2026-08-06T10:00:00Z",
    reviewedItems: 2214,
    reviewedPotentialCleanup: "32.70GB",
    systemScope: "userOnly",
    completion: "finished",
    warnings: [],
    executedAtMs: atMs,
    logError: null,
  }) as never;

const deleted = (id: number, atMs: number, extra: Record<string, unknown> = {}) =>
  ({
    id,
    backend: "rip",
    targetPath: `/Users/person/tmp/gone-${id}`,
    disposition: "trash",
    recoverable: true,
    undone: false,
    deletedAtMs: atMs,
    undoneAtMs: null,
    logError: null,
    ...extra,
  }) as never;

test("an empty journal set is an empty timeline, not a loading state", () => {
  assert.deepEqual(mergeActivity(NO_JOURNALS), []);
  assert.deepEqual(activityCounts([]), { total: 0, trashed: 0, cleaned: 0, deleted: 0 });
});

test("all three journals reach the timeline, newest first", () => {
  const entries = mergeActivity({
    trashed: [trashed(4, 400)],
    cleaned: [cleaned(2, 200)],
    deleted: [deleted(6, 600)],
  });
  assert.deepEqual(
    entries.map((entry) => [entry.kind, entry.id]),
    [
      ["deleted", 6],
      ["trashed", 4],
      ["cleaned", 2],
    ],
  );
});

test("two events in the same millisecond order by the id they share a counter with", () => {
  const entries = mergeActivity({
    trashed: [trashed(5, 900), trashed(9, 900)],
    cleaned: [cleaned(7, 900)],
    deleted: [],
  });
  assert.deepEqual(
    entries.map((entry) => entry.id),
    [9, 7, 5],
  );
});

test("counts report each kind separately, because they are not the same operation", () => {
  const entries = mergeActivity({
    trashed: [trashed(1, 100), trashed(2, 200)],
    cleaned: [cleaned(3, 300)],
    deleted: [deleted(4, 400)],
  });
  assert.deepEqual(activityCounts(entries), { total: 4, trashed: 2, cleaned: 1, deleted: 1 });
});

test("recovery says what the actual route back is for each kind", () => {
  const [trash, clean, undoable, permanent, undone] = mergeActivity({
    trashed: [trashed(50, 500)],
    cleaned: [cleaned(40, 400)],
    deleted: [
      deleted(30, 300),
      deleted(20, 200, { recoverable: false }),
      deleted(10, 100, { undone: true, undoneAtMs: 150 }),
    ],
  }) as ActivityEntry[];
  assert.equal(recoveryOf(trash!), "putBack");
  assert.equal(recoveryOf(clean!), "none");
  assert.equal(recoveryOf(undoable!), "undoable");
  assert.equal(recoveryOf(permanent!), "none");
  assert.equal(recoveryOf(undone!), "undone");
});

test("only measured sizes are summed, so no label becomes arithmetic", () => {
  const entries = mergeActivity({
    trashed: [trashed(1, 100, 4096), trashed(2, 200, 1024)],
    cleaned: [cleaned(3, 300)],
    deleted: [deleted(4, 400)],
  });
  assert.equal(measuredBytes(entries), 5120);
});

test("a journal write that failed travels with the entry it belongs to", () => {
  const entries = mergeActivity({
    trashed: [trashed(1, 100)],
    cleaned: [],
    deleted: [],
  });
  assert.equal(entries[0]!.operation.logError, null);
  const withError = mergeActivity({
    trashed: [{ ...(trashed(1, 100) as object), logError: "disk full" } as never],
    cleaned: [],
    deleted: [],
  });
  assert.equal(withError[0]!.operation.logError, "disk full");
});
