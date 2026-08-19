import assert from "node:assert/strict";
import test from "node:test";

import {
  canStartScan,
  INITIAL_SCAN,
  reduceScan,
  scanStatusLine,
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

/**
 * The strip sits above the page, so its height is the page's position. Every
 * assertion here is really about that: one line, one label, and the part that
 * can be long kept in the field that truncates.
 */
test("the status strip is one line, with anything long in the truncating half", () => {
  const deep = "/Users/sc/Projects/nirmoka/node_modules/.pnpm/@jridgewell+gen-mapping@0.3.13/dist";
  const scanning = run([
    { type: "requested", root: "~" },
    { type: "progress", progress: { scanned: 1_100_001, currentPath: deep } as never },
  ]);
  const line = scanStatusLine({
    state: scanning,
    scanner: "ncdu",
    backendError: null,
    formatCount: (value) => value.toLocaleString("en-US"),
  });

  assert.equal(line?.label, "Scanning 1,100,001 entries…");
  assert.equal(line?.detail, deep, "the path is the detail, because that is what gets cut");
  assert.equal(line?.tone, "muted");
  assert.equal(line?.label.includes("\n"), false);
});

test("a finished scan reports its numbers and its root, in that order", () => {
  const line = scanStatusLine({
    state: run([{ type: "restored", summary: summary(1) }]),
    scanner: "ncdu",
    backendError: null,
    formatCount: (value) => value.toLocaleString("en-US"),
  });

  assert.equal(line?.label, "2,200,000 entries · ncdu");
  assert.equal(line?.detail, "/Users/fixture");
});

test("a problem arrives as the detail rather than as a second line", () => {
  const failed = scanStatusLine({
    state: run([
      { type: "requested", root: "~" },
      { type: "failed", failure: { message: "ncdu exited with status 1", cancelled: false } },
    ]),
    scanner: "ncdu",
    backendError: null,
    formatCount: String,
  });
  assert.deepEqual(failed, {
    label: "Scan failed.",
    detail: "ncdu exited with status 1",
    tone: "error",
  });

  const backendBroken = scanStatusLine({
    state: run([{ type: "restored", summary: summary(2) }]),
    scanner: "ncdu",
    backendError: "detection failed",
    formatCount: String,
  });
  assert.equal(backendBroken?.detail, "detection failed");
  assert.equal(backendBroken?.tone, "error");
});

test("an idle window with a usable scanner says nothing, so no strip is drawn", () => {
  assert.equal(
    scanStatusLine({
      state: INITIAL_SCAN,
      scanner: "ncdu",
      backendError: null,
      formatCount: String,
    }),
    null,
  );
  assert.equal(
    scanStatusLine({
      state: INITIAL_SCAN,
      scanner: undefined,
      backendError: null,
      formatCount: String,
    })?.label,
    "No scanner installed.",
  );
});

/** Progress for a scan that already finished must not reopen the strip. */
test("the strip follows the state machine rather than the last event", () => {
  const late = run([
    { type: "requested", root: "~" },
    { type: "finished", summary: summary(3) },
    { type: "progress", progress: { scanned: 99, currentPath: "/late" } as never },
  ]);
  const line = scanStatusLine({
    state: late,
    scanner: "ncdu",
    backendError: null,
    formatCount: String,
  });
  assert.equal(line?.detail, "/Users/fixture");
});
