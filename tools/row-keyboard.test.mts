import assert from "node:assert/strict";
import test from "node:test";

import { PAGE, rowIntent, rowLabel } from "../apps/desktop/src/components/row-keyboard.ts";

const list = (selected: number | null, total = 1000) => ({ selected, total });

test("arrows move one row and stop at the ends", () => {
  assert.deepEqual(rowIntent("ArrowDown", list(0)), { kind: "select", index: 1 });
  assert.deepEqual(rowIntent("ArrowUp", list(5)), { kind: "select", index: 4 });
  assert.deepEqual(rowIntent("ArrowUp", list(0)), { kind: "select", index: 0 });
  assert.deepEqual(rowIntent("ArrowDown", list(999)), { kind: "select", index: 999 });
});

test("the first keystroke selects an end rather than the second row", () => {
  assert.deepEqual(rowIntent("ArrowDown", list(null)), { kind: "select", index: 0 });
  assert.deepEqual(rowIntent("ArrowUp", list(null)), { kind: "select", index: 999 });
  assert.deepEqual(rowIntent("PageDown", list(null)), { kind: "select", index: 0 });
});

test("page and home keys jump without leaving the directory", () => {
  assert.deepEqual(rowIntent("PageDown", list(0)), { kind: "select", index: PAGE });
  assert.deepEqual(rowIntent("PageUp", list(PAGE + 3)), { kind: "select", index: 3 });
  assert.deepEqual(rowIntent("PageUp", list(2)), { kind: "select", index: 0 });
  assert.deepEqual(rowIntent("PageDown", list(995)), { kind: "select", index: 999 });
  assert.deepEqual(rowIntent("Home", list(500)), { kind: "select", index: 0 });
  assert.deepEqual(rowIntent("End", list(500)), { kind: "select", index: 999 });
});

test("a directory of one row has nowhere to move", () => {
  for (const key of ["ArrowDown", "ArrowUp", "PageDown", "PageUp", "Home", "End"]) {
    assert.deepEqual(rowIntent(key, list(0, 1)), { kind: "select", index: 0 }, key);
  }
});

test("open, up, and history keys map to their intents", () => {
  assert.deepEqual(rowIntent("Enter", list(3)), { kind: "open" });
  assert.deepEqual(rowIntent("ArrowRight", list(3)), { kind: "open" });
  assert.deepEqual(rowIntent(" ", list(3)), { kind: "preview" });
  assert.deepEqual(rowIntent("ArrowLeft", list(3)), { kind: "up" });
  assert.deepEqual(rowIntent("Backspace", list(3)), { kind: "up" });
  assert.deepEqual(rowIntent("BrowserBack", list(3)), { kind: "back" });
  assert.deepEqual(rowIntent("BrowserForward", list(3)), { kind: "forward" });
});

test("keys that act on a row do nothing until one is selected", () => {
  assert.equal(rowIntent("Enter", list(null)), null);
  assert.equal(rowIntent(" ", list(null)), null);
});

/** An empty directory is still a place you have to be able to leave. */
test("an empty directory keeps the keys that leave it and drops the rest", () => {
  assert.deepEqual(rowIntent("ArrowLeft", list(null, 0)), { kind: "up" });
  assert.deepEqual(rowIntent("Backspace", list(null, 0)), { kind: "up" });
  assert.deepEqual(rowIntent("BrowserBack", list(null, 0)), { kind: "back" });
  assert.equal(rowIntent("ArrowDown", list(null, 0)), null);
  assert.equal(rowIntent("Home", list(null, 0)), null);
  assert.equal(rowIntent("Enter", list(null, 0)), null);
});

/** `null` is what tells the caller to leave the event alone. */
test("unhandled keys are reported as unhandled", () => {
  for (const key of ["a", "Tab", "Escape", "F5", "/"]) {
    assert.equal(rowIntent(key, list(2)), null, key);
  }
});

test("a row label says what makes its number unreliable", () => {
  const base = {
    name: "Library",
    kind: "directory",
    size: "4.20 GB",
    childCount: 12,
    readError: false,
    excluded: false,
    hardlink: false,
  };

  assert.equal(rowLabel(base), "Folder Library, 4.20 GB, 12 entries");
  assert.equal(
    rowLabel({ ...base, kind: "file", childCount: 0, name: "big.bin" }),
    "File big.bin, 4.20 GB",
  );
  assert.equal(rowLabel({ ...base, childCount: 1 }), "Folder Library, 4.20 GB, 1 entry");

  const flagged = rowLabel({ ...base, readError: true, excluded: true, hardlink: true });
  assert.match(flagged, /could not be read, size is a lower bound/);
  assert.match(flagged, /excluded from the scan/);
  assert.match(flagged, /hardlink, counted once elsewhere/);
});
