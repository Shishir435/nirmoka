import assert from "node:assert/strict";
import test from "node:test";

import { CHUNK, chunkOffsets } from "../apps/desktop/src/hooks/chunk-window.ts";

test("a visible range asks for the chunks it covers and no others", () => {
  assert.deepEqual(chunkOffsets(0, 20, 1000), [0]);
  assert.deepEqual(chunkOffsets(0, CHUNK, 1000), [0, 100]);
  assert.deepEqual(chunkOffsets(150, 260, 1000), [100, 200]);
});

/**
 * The case invariant 5 exists for. Scrolling deep into a quarter of a million
 * rows must ask for the window on screen, not everything up to it.
 */
test("a huge directory still asks for one window at a time", () => {
  const offsets = chunkOffsets(249_000, 249_030, 250_000);

  assert.deepEqual(offsets, [249_000]);
  assert.equal(chunkOffsets(0, 11, 250_000).length, 1);
});

test("a range past the end is clamped rather than asked for", () => {
  assert.deepEqual(chunkOffsets(90, 400, 120), [0, 100]);
  assert.deepEqual(chunkOffsets(500, 900, 120), [100], "clamped to the last row");
});

test("a directory with no rows asks for nothing", () => {
  assert.deepEqual(chunkOffsets(0, 50, 0), []);
  assert.deepEqual(chunkOffsets(0, 0, -1), []);
});

test("a backwards or negative range is not a request", () => {
  assert.deepEqual(chunkOffsets(50, 10, 1000), [], "end before start");
  assert.deepEqual(chunkOffsets(-40, -1, 1000), [0], "clamped to the first row");
});

test("offsets are chunk-aligned, because that is what a request takes", () => {
  for (const offset of chunkOffsets(0, 999, 1000)) {
    assert.equal(offset % CHUNK, 0, `${offset} is not the start of a chunk`);
  }
});
