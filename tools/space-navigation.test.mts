import assert from "node:assert/strict";
import test from "node:test";

import {
  canGoBack,
  canGoForward,
  currentLocation,
  EMPTY_HISTORY,
  goBack,
  goForward,
  parentIdForScan,
  visit,
} from "../apps/desktop/src/pages/space-navigation.ts";

test("directory navigation is retained only for the scan that created it", () => {
  const location = { scanId: 41, parentId: 7 };

  assert.equal(parentIdForScan(location, 41), 7);
  assert.equal(parentIdForScan(location, 42), null);
  assert.equal(parentIdForScan(location, null), null);
});

test("back and forward walk the directories that were visited", () => {
  let history = EMPTY_HISTORY;
  assert.equal(currentLocation(history), null);
  assert.equal(canGoBack(history), false);
  assert.equal(canGoForward(history), false);

  for (const parentId of [null, 4, 9]) {
    history = visit(history, { scanId: 1, parentId });
  }
  assert.deepEqual(currentLocation(history), { scanId: 1, parentId: 9 });
  assert.equal(canGoForward(history), false);

  history = goBack(history);
  assert.deepEqual(currentLocation(history), { scanId: 1, parentId: 4 });
  assert.equal(canGoBack(history), true);
  assert.equal(canGoForward(history), true);

  history = goForward(history);
  assert.deepEqual(currentLocation(history), { scanId: 1, parentId: 9 });

  history = goBack(goBack(history));
  assert.deepEqual(currentLocation(history), { scanId: 1, parentId: null });
  assert.equal(canGoBack(history), false);
  // At either end the call is a no-op rather than an error: the buttons are
  // disabled, and a keystroke should not need to know that.
  assert.equal(goBack(history), history);
});

test("opening a directory discards forward history", () => {
  let history = EMPTY_HISTORY;
  for (const parentId of [null, 4, 9]) {
    history = visit(history, { scanId: 1, parentId });
  }
  history = goBack(goBack(history));

  history = visit(history, { scanId: 1, parentId: 12 });

  assert.deepEqual(currentLocation(history), { scanId: 1, parentId: 12 });
  assert.equal(canGoForward(history), false, "the abandoned branch must not come back");
  assert.deepEqual(
    history.entries.map((entry) => entry.parentId),
    [null, 12],
  );
});

test("reopening the current directory does not grow the history", () => {
  const history = visit(visit(EMPTY_HISTORY, { scanId: 1, parentId: 3 }), {
    scanId: 1,
    parentId: 3,
  });

  assert.equal(history.entries.length, 1);
  assert.equal(canGoBack(history), false, "back would otherwise appear to do nothing");
});

/**
 * Node ids are per-scan arena indices, so history from a replaced scan names
 * different directories. The whole point of keeping the scan id in the entry.
 */
test("a location from another scan starts a fresh history", () => {
  let history = EMPTY_HISTORY;
  for (const parentId of [null, 4]) {
    history = visit(history, { scanId: 1, parentId });
  }

  history = visit(history, { scanId: 2, parentId: null });

  assert.deepEqual(history, { entries: [{ scanId: 2, parentId: null }], index: 0 });
  assert.equal(canGoBack(history), false);
});
