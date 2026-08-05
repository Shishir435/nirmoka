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
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
    { type: "runStarted" },
    { type: "trashed", operation: operation() },
  ]);

  assert.deepEqual(state.trashedIds, [42]);
  assert.equal(state.running, false);
  assert.equal(state.preparation, null);
  assert.equal(state.error, null);
});

test("the token is dropped when the run starts, not when it finishes", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
    { type: "runStarted" },
  ]);

  // Rust consumes the token on receipt. A dialog still holding it would let a
  // second click submit a confirmation that no longer exists.
  assert.equal(state.preparation, null);
  assert.equal(state.running, true);
});

test("a failed move leaves the row unmarked", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
    { type: "runStarted" },
    { type: "runFailed", message: "macOS did not let Nirmoka ask the Finder" },
  ]);

  assert.deepEqual(state.trashedIds, [], "nothing moved, so nothing is marked");
  assert.match(state.error ?? "", /did not let Nirmoka/);
});

test("a refused preparation opens no dialog", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepareFailed", nodeId: 42, message: "cannot delete the scan root" },
  ]);

  assert.equal(state.preparation, null);
  assert.equal(state.pendingNodeId, null);
  assert.match(state.error ?? "", /scan root/);
});

test("a row already in the Trash cannot be sent there twice", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
    { type: "runStarted" },
    { type: "trashed", operation: operation() },
  ]);

  assert.equal(canTrash(state, row(42)), false);
  assert.equal(isTrashed(state, row(42)), true);
  assert.equal(canTrash(state, row(43)), true);
});

test("nothing is offered while a confirmation is open or a move is running", () => {
  const open = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
  ]);
  assert.equal(canTrash(open, row(9)), false);

  const running = reduceTrash(open, { type: "runStarted" });
  assert.equal(canTrash(running, row(9)), false);

  assert.equal(canTrash(INITIAL_TRASH, undefined), false, "no selection, no action");
});

test("changing directory clears the error but keeps what was moved", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
    { type: "runStarted" },
    { type: "trashed", operation: operation() },
    { type: "prepareStarted", nodeId: 43 },
    { type: "prepareFailed", nodeId: 43, message: "cannot be resolved" },
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
    { type: "prepareStarted", nodeId: 42 },
    { type: "moved" },
    { type: "prepared", nodeId: 42, preparation: preparation() },
  ]);

  assert.equal(state.preparation, null, "no dialog for a row that is not on screen");
  assert.equal(state.preparing, false);
  assert.equal(state.pendingNodeId, null);
});

test("a refusal that arrives after the user left says nothing", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "moved" },
    { type: "prepareFailed", nodeId: 42, message: "cannot be resolved" },
  ]);

  assert.equal(state.error, null, "an error under a directory nobody is looking at");
});

test("a stale refusal does not cancel the request that replaced it", () => {
  // `preparing` is true again the moment a replacement starts, so gating on it
  // alone lets an abandoned request's rejection clear the replacement's row —
  // and the replacement's own answer is then dropped as unexpected, leaving no
  // dialog and someone else's error.
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "moved" },
    { type: "prepareStarted", nodeId: 43 },
    { type: "prepareFailed", nodeId: 42, message: "cannot be resolved" },
    { type: "prepared", nodeId: 43, preparation: preparation(5) },
  ]);

  assert.equal(state.error, null, "the abandoned request's refusal is not shown");
  assert.equal(state.preparation?.confirmationToken, 5, "the live request still opens its dialog");
});

test("an answer to a superseded request cannot replace a newer one", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "moved" },
    { type: "prepareStarted", nodeId: 43 },
    // The first request finally lands, after a second one was started.
    { type: "prepared", nodeId: 42, preparation: preparation(1) },
    { type: "prepared", nodeId: 43, preparation: preparation(2) },
  ]);

  assert.equal(state.preparation?.confirmationToken, 2, "the row that was actually asked about");
  assert.equal(state.pendingNodeId, 43);
});

test("leaving a directory does not abandon a move already underway", () => {
  // The item is between here and the Trash. The result still has to mark its
  // row and reach the journal.
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
    { type: "runStarted" },
    { type: "moved" },
    { type: "trashed", operation: operation() },
  ]);

  assert.deepEqual(state.trashedIds, [42]);
  assert.equal(state.running, false);
});

test("a rescan forgets every id, because the tree renumbers from zero", () => {
  const state = run([
    { type: "prepareStarted", nodeId: 42 },
    { type: "prepared", nodeId: 42, preparation: preparation() },
    { type: "runStarted" },
    { type: "trashed", operation: operation() },
    { type: "rescanned" },
  ]);

  assert.deepEqual(state, INITIAL_TRASH);
});

test("a move whose journal write failed still reports the move", () => {
  const message = outcomeMessage(operation("permission denied"));

  assert.match(message, /is in the Trash/);
  assert.match(message, /could not be added to the operation log/);
  assert.match(outcomeMessage(operation()), /Put Back/);
});
