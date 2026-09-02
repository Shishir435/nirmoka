import assert from "node:assert/strict";
import test from "node:test";

import {
  canTrash,
  INITIAL_TRASH,
  isTrashed,
  outcomeMessage,
  reduceTrash,
  type TrashEvent,
  type TrashState,
} from "../apps/desktop/src/lib/engine/trash-flow.ts";

const preparation = (token = 7) =>
  ({
    confirmationToken: token,
    targetPath: "/Users/person/Projects/old",
    totalBytes: 4096,
    isDirectory: true,
    warning: "This folder and everything inside it moves to the Trash.",
  }) as never;

const operation = (logError: string | null = null) =>
  ({
    id: 3,
    targetPath: "/Users/person/Projects/old",
    totalBytes: 4096,
    trashedAtMs: 1_760_000_000_000,
    logError,
  }) as never;

const row = (id: number) => ({ id, name: "old", kind: "directory" }) as never;

const run = (events: TrashEvent[], from: TrashState = INITIAL_TRASH) =>
  events.reduce(reduceTrash, from);

test("a confirmed move marks the row it was prepared for", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 3 },
    { type: "trashed", requestId: 3, operation: operation() },
  ]);

  assert.deepEqual(state.trashedIds, [42]);
  assert.equal(state.running, false);
  assert.equal(state.preparation, null);
  assert.equal(state.error, null);
});

test("the token is dropped when the run starts, not when it finishes", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 3 },
  ]);

  // Rust consumes the token on receipt. A dialog still holding it would let a
  // second click submit a confirmation that no longer exists.
  assert.equal(state.preparation, null);
  assert.equal(state.running, true);
});

test("a failed move leaves the row unmarked", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 3 },
    { type: "runFailed", requestId: 3, message: "macOS did not let Nirmoka ask the Finder" },
  ]);

  assert.deepEqual(state.trashedIds, [], "nothing moved, so nothing is marked");
  assert.match(state.error ?? "", /did not let Nirmoka/u);
});

test("a refused preparation opens no dialog", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepareFailed", requestId: 1, message: "cannot delete the scan root" },
  ]);

  assert.equal(state.preparation, null);
  assert.equal(state.pendingNodeId, null);
  assert.match(state.error ?? "", /scan root/u);
});

test("a row already in the Trash cannot be sent there twice", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 3 },
    { type: "trashed", requestId: 3, operation: operation() },
  ]);

  assert.equal(canTrash(state, row(42)), false);
  assert.equal(isTrashed(state, row(42)), true);
  assert.equal(canTrash(state, row(43)), true);
});

test("nothing is offered while a confirmation is open or a move is running", () => {
  const open = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
  ]);
  assert.equal(canTrash(open, row(9)), false);

  const running = reduceTrash(open, { type: "runStarted", requestId: 3 });
  assert.equal(canTrash(running, row(9)), false);

  assert.equal(canTrash(INITIAL_TRASH, undefined), false, "no selection, no action");
});

test("changing directory clears the error but keeps what was moved", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 3 },
    { type: "trashed", requestId: 3, operation: operation() },
    { type: "prepareStarted", requestId: 2, nodeId: 43 },
    { type: "prepareFailed", requestId: 2, message: "cannot be resolved" },
    { type: "moved" },
  ]);

  assert.equal(state.error, null);
  assert.deepEqual(state.trashedIds, [42], "those rows are still in the Trash");
});

test("a preparation that arrives after the user left opens nothing", () => {
  // The round trip outlives the directory. Rust answers, and by then the user
  // is somewhere else — but navigating within a scan does not change the scan
  // id `confirm_trash` checks, so the token would still have executed.
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "moved" },
    { type: "prepared", requestId: 1, preparation: preparation() },
  ]);

  assert.equal(state.preparation, null, "no dialog for a row that is not on screen");
  assert.equal(state.preparing, false);
  assert.equal(state.pendingNodeId, null);
});

test("a refusal that arrives after the user left says nothing", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "moved" },
    { type: "prepareFailed", requestId: 1, message: "cannot be resolved" },
  ]);

  assert.equal(state.error, null, "an error under a directory nobody is looking at");
});

test("a stale refusal does not cancel the request that replaced it", () => {
  // `preparing` is true again the moment a replacement starts, so gating on it
  // alone lets an abandoned request's rejection clear the replacement's row —
  // and the replacement's own answer is then dropped as unexpected, leaving no
  // dialog and someone else's error.
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "moved" },
    { type: "prepareStarted", requestId: 2, nodeId: 43 },
    { type: "prepareFailed", requestId: 1, message: "cannot be resolved" },
    { type: "prepared", requestId: 2, preparation: preparation(5) },
  ]);

  assert.equal(state.error, null, "the abandoned request's refusal is not shown");
  assert.equal(state.preparation?.confirmationToken, 5, "the live request still opens its dialog");
});

test("an answer to a superseded request cannot replace a newer one", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "moved" },
    { type: "prepareStarted", requestId: 2, nodeId: 43 },
    // The first request finally lands, after a second one was started.
    { type: "prepared", requestId: 1, preparation: preparation(1) },
    { type: "prepared", requestId: 2, preparation: preparation(2) },
  ]);

  assert.equal(state.preparation?.confirmationToken, 2, "the row that was actually asked about");
  assert.equal(state.pendingNodeId, 43);
});

test("leaving a directory does not abandon a move already underway", () => {
  // The item is between here and the Trash. The result still has to mark its
  // row and reach the journal.
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 3 },
    { type: "moved" },
    { type: "trashed", requestId: 3, operation: operation() },
  ]);

  assert.deepEqual(state.trashedIds, [42]);
  assert.equal(state.running, false);
});

test("asking about the same row twice does not confuse the two requests", () => {
  // Navigate away and back, click the same row again, and the second request
  // wears the first one's node id. Whichever way the two replies interleave,
  // only the live one may answer.
  const failThenSucceed = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "moved" },
    { type: "prepareStarted", requestId: 2, nodeId: 42 },
    { type: "prepareFailed", requestId: 1, message: "cannot be resolved" },
    { type: "prepared", requestId: 2, preparation: preparation(9) },
  ]);

  assert.equal(failThenSucceed.error, null, "the abandoned request's refusal is not shown");
  assert.equal(failThenSucceed.preparation?.confirmationToken, 9);
  assert.equal(failThenSucceed.pendingNodeId, 42);

  const succeedTwice = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "moved" },
    { type: "prepareStarted", requestId: 2, nodeId: 42 },
    // The abandoned request's token was already superseded inside Rust, so
    // this dialog would have failed on confirm.
    { type: "prepared", requestId: 1, preparation: preparation(1) },
    { type: "prepared", requestId: 2, preparation: preparation(2) },
  ]);

  assert.equal(succeedTwice.preparation?.confirmationToken, 2, "the live token, not the stale one");
});

test("a move that outlives a rescan does not mark whatever came next", () => {
  // The largest window in the whole flow: on macOS the move asks the Finder,
  // which can sit on a permission prompt for as long as the user ignores it.
  // A rescan and a second move fit inside that comfortably.
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 2 },
    { type: "rescanned" },
    // A different tree, a different row that happens to reuse the id.
    { type: "prepareStarted", requestId: 3, nodeId: 42 },
    { type: "prepared", requestId: 3, preparation: preparation(8) },
    { type: "runStarted", requestId: 4 },
    // The first move finally reports.
    { type: "trashed", requestId: 2, operation: operation() },
  ]);

  assert.deepEqual(state.trashedIds, [], "the new row is not in the Trash");
  assert.equal(state.running, true, "the move that is actually running still is");
  assert.equal(state.last, null);
});

test("a stale failure does not re-enable the buttons mid-move", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 2 },
    { type: "rescanned" },
    { type: "prepareStarted", requestId: 3, nodeId: 7 },
    { type: "prepared", requestId: 3, preparation: preparation(8) },
    { type: "runStarted", requestId: 4 },
    { type: "runFailed", requestId: 2, message: "the Finder said no" },
  ]);

  assert.equal(state.running, true, "a real move is still in flight");
  assert.equal(state.error, null, "and nobody is told otherwise");
  assert.equal(canTrash(state, row(9)), false, "so nothing else can be started");
});

test("a reply that outlives a rescan is not answered", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "rescanned" },
    { type: "prepared", requestId: 1, preparation: preparation() },
  ]);

  assert.deepEqual(state, INITIAL_TRASH, "a new tree owes the old one nothing");
});

test("a rescan forgets every id, because the tree renumbers from zero", () => {
  const state = run([
    { type: "prepareStarted", requestId: 1, nodeId: 42 },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 3 },
    { type: "trashed", requestId: 3, operation: operation() },
    { type: "rescanned" },
  ]);

  assert.deepEqual(state, INITIAL_TRASH);
});

test("a move whose journal write failed still reports the move", () => {
  const message = outcomeMessage(operation("permission denied"));

  assert.match(message, /is in the Trash/u);
  assert.match(message, /could not be added to the operation log/u);
  assert.match(outcomeMessage(operation()), /Put Back/u);
});
