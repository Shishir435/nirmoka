import assert from "node:assert/strict";
import test from "node:test";

import {
  canStartScan,
  INITIAL_SCAN,
  reduceScan,
  type ScanEvent,
  type ScanState,
} from "../apps/desktop/src/lib/engine/scan-machine.ts";

const summary = (scanId: number) =>
  ({
    scanId,
    rootPath: "/Users/fixture",
    backendId: "ncdu",
    entries: 2_200_000,
    directories: 180_000,
    totalBytes: 421_000_000_000,
    apparentBytes: 430_000_000_000,
    readErrors: 3,
    excluded: 0,
    hardlinksDeduplicated: 0,
    hardlinkBytesSaved: 0,
  }) as never;

const run = (events: ScanEvent[], from: ScanState = INITIAL_SCAN) =>
  events.reduce(reduceScan, from);

test("a scan reports the canonical root Rust answered with", () => {
  const state = run([
    { type: "requested", root: "~" },
    { type: "rooted", root: "/Users/fixture" },
  ]);

  assert.equal(state.status, "scanning");
  assert.equal(state.status === "scanning" && state.root, "/Users/fixture");
  assert.equal(state.status === "scanning" && state.progress.currentPath, "/Users/fixture");
});

test("progress counts up while scanning and finishing ends it", () => {
  const scanning = run([
    { type: "requested", root: "~" },
    { type: "progress", progress: { scanned: 25_000, currentPath: "/Users/fixture/Library" } },
  ]);
  assert.equal(scanning.status === "scanning" && scanning.progress.scanned, 25_000);

  const done = reduceScan(scanning, { type: "finished", summary: summary(1) });
  assert.equal(done.status, "done");
});

/**
 * The event that arrives after the scan it belongs to has ended. Applying it
 * would reopen a finished scan and put the window back on "scanning".
 */
test("late progress cannot reopen a finished scan", () => {
  const done = run([
    { type: "requested", root: "~" },
    { type: "finished", summary: summary(1) },
  ]);

  const after = reduceScan(done, {
    type: "progress",
    progress: { scanned: 99, currentPath: "/late" },
  });
  assert.equal(after, done);

  const rooted = reduceScan(done, { type: "rooted", root: "/late" });
  assert.equal(rooted, done);
});

test("cancelling is not a failure", () => {
  const cancelled = run([
    { type: "requested", root: "~" },
    { type: "failed", failure: { message: "cancelled by user", cancelled: true } },
  ]);
  assert.equal(cancelled.status, "cancelled");

  const failed = run([
    { type: "requested", root: "~" },
    { type: "failed", failure: { message: "ncdu is not installed", cancelled: false } },
  ]);
  assert.equal(failed.status, "failed");
  assert.equal(failed.status === "failed" && failed.message, "ncdu is not installed");
});

test("a rescan starts clean rather than inheriting the previous numbers", () => {
  const rescanned = run([
    { type: "requested", root: "~" },
    { type: "progress", progress: { scanned: 400_000, currentPath: "/deep" } },
    { type: "failed", failure: { message: "stopped", cancelled: true } },
    { type: "requested", root: "/Volumes/Data" },
  ]);

  assert.equal(rescanned.status, "scanning");
  assert.equal(rescanned.status === "scanning" && rescanned.progress.scanned, 0);
  assert.equal(rescanned.status === "scanning" && rescanned.root, "/Volumes/Data");
});

test("a scan found at startup is a completed scan", () => {
  const restored = reduceScan(INITIAL_SCAN, { type: "restored", summary: summary(7) });

  assert.equal(restored.status, "done");
});

/**
 * Both halves are real failures: no scanner means the command has nothing to
 * run, and starting before the listeners are registered loses the terminal
 * event and leaves the window scanning forever.
 */
test("the scan button needs a scanner, listeners, and no scan in flight", () => {
  const idle = INITIAL_SCAN;
  assert.equal(canStartScan({ scanner: "ncdu", listenersReady: true, state: idle }), true);
  assert.equal(canStartScan({ scanner: null, listenersReady: true, state: idle }), false);
  assert.equal(canStartScan({ scanner: undefined, listenersReady: true, state: idle }), false);
  assert.equal(canStartScan({ scanner: "ncdu", listenersReady: false, state: idle }), false);

  const scanning = reduceScan(idle, { type: "requested", root: "~" });
  assert.equal(canStartScan({ scanner: "ncdu", listenersReady: true, state: scanning }), false);

  const cancelled = reduceScan(scanning, {
    type: "failed",
    failure: { message: "stopped", cancelled: true },
  });
  assert.equal(
    canStartScan({ scanner: "ncdu", listenersReady: true, state: cancelled }),
    true,
    "a cancelled scan must be restartable",
  );
});
