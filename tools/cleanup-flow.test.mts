import assert from "node:assert/strict";
import test from "node:test";

import {
  canReview,
  INITIAL_CLEANUP,
  outcomeLabel,
  outcomeTone,
  reduceCleanup,
  type CleanupEvent,
  type CleanupState,
} from "../apps/desktop/src/lib/engine/cleanup-flow.ts";

const preview = (totalItems: number) =>
  ({
    backend: "mole",
    backendInsteadOf: null,
    backendVersion: "1.48.1",
    generatedAt: "2026-08-03 12:30:00",
    categories: [],
    potentialCleanup: "At least 192.00MB",
    totalItems,
    systemScope: "userOnly",
    warnings: [],
  }) as never;

const preparation = () =>
  ({
    confirmationToken: 4,
    backend: "mole",
    backendInsteadOf: null,
    backendVersion: "1.48.1",
    previewGeneratedAt: "2026-08-03 12:30:00",
    potentialCleanup: "At least 192.00MB",
    totalItems: 6,
    systemScope: "userOnly",
    warnings: [],
    expiresInSeconds: 300,
    requiresConfirmation: true,
    warning: "Mole will re-discover eligible candidates during execution.",
  }) as never;

const operation = (completion: string) =>
  ({
    id: 1,
    backend: "mole",
    backendVersion: "1.48.1",
    previewGeneratedAt: "2026-08-03 12:30:00",
    reviewedItems: 6,
    reviewedPotentialCleanup: "At least 192.00MB",
    systemScope: "userOnly",
    completion,
    warnings: [],
    executedAtMs: 1_785_000_000_000,
    logError: null,
  }) as never;

const run = (events: CleanupEvent[], from: CleanupState = INITIAL_CLEANUP) =>
  events.reduce(reduceCleanup, from);

test("a review is only offered for a preview with something in it", () => {
  assert.equal(canReview(INITIAL_CLEANUP), false);

  const empty = run([{ type: "previewArrived", preview: preview(0) }]);
  assert.equal(canReview(empty), false, "an empty plan is nothing to confirm");

  const ready = run([{ type: "previewArrived", preview: preview(6) }]);
  assert.equal(canReview(ready), true);

  const previewing = run([{ type: "previewStarted" }], ready);
  assert.equal(canReview(previewing), false, "a refresh is in flight");
});

test("the whole path from preview to a finished run", () => {
  const finished = run([
    { type: "previewStarted" },
    { type: "previewArrived", preview: preview(6) },
    { type: "reviewed", preparation: preparation() },
    { type: "runStarted" },
    { type: "runFinished", operation: operation("finished") },
  ]);

  assert.equal(finished.running, false);
  assert.equal(finished.preparation, null);
  assert.equal(finished.result?.completion, "finished");
  assert.equal(finished.runError, null);
});

/**
 * Mole re-discovers candidates when it runs, and Rust drops the review as soon
 * as execution is confirmed (ADR 0020). A window still listing those paths would
 * be presenting a past discovery as the current one.
 */
test("starting a run spends the reviewed preview", () => {
  const running = run([
    { type: "previewArrived", preview: preview(6) },
    { type: "reviewed", preparation: preparation() },
    { type: "runStarted" },
  ]);

  assert.equal(running.preview, null);
  assert.equal(running.running, true);
  assert.equal(canReview(running), false, "there is nothing left to confirm");
});

test("stopping is only possible during a run, and the run still reports", () => {
  const idle = run([{ type: "stopRequested" }]);
  assert.equal(idle.stopping, false, "nothing to stop");

  const stopped = run([
    { type: "previewArrived", preview: preview(6) },
    { type: "reviewed", preparation: preparation() },
    { type: "runStarted" },
    { type: "stopRequested" },
    { type: "runFinished", operation: operation("cancelled") },
  ]);

  assert.equal(stopped.stopping, false);
  assert.equal(stopped.running, false);
  assert.equal(stopped.result?.completion, "cancelled", "a stopped run is a result, not an error");
  assert.equal(stopped.runError, null);
});

test("a run that never started leaves an error and no result", () => {
  const refused = run([
    { type: "previewArrived", preview: preview(6) },
    { type: "reviewed", preparation: preparation() },
    { type: "runStarted" },
    { type: "runFailed", message: "mo changed from reviewed version 1.48.1 to 1.49.0" },
  ]);

  assert.equal(refused.result, null);
  assert.match(refused.runError ?? "", /changed from reviewed version/);
  assert.equal(refused.running, false);
  assert.equal(refused.preview, null, "the spent review is gone either way");
});

test("a confirmation that could not be prepared clears the stale preview", () => {
  const stale = run([
    { type: "previewArrived", preview: preview(6) },
    { type: "reviewFailed", message: "no fresh non-empty cleanup preview is available" },
  ]);

  assert.equal(stale.preparation, null);
  assert.equal(stale.preview, null);
  assert.equal(canReview(stale), false);
});

test("dismissing the dialog keeps the review for another try", () => {
  const dismissed = run([
    { type: "previewArrived", preview: preview(6) },
    { type: "reviewed", preparation: preparation() },
    { type: "reviewDismissed" },
  ]);

  assert.equal(dismissed.preparation, null);
  assert.equal(canReview(dismissed), true);
});

/**
 * Cancelling a discovery comes back as a rejected request, so the ending that
 * clears "Stopping preview" is the failure path. Leaving the flag set disables
 * the only button that can start another one.
 */
test("stopping a preview never outlives the preview", () => {
  const cancelled = run([
    { type: "previewStarted" },
    { type: "previewStopRequested" },
    { type: "previewFailed", message: "cancelled" },
  ]);

  assert.equal(cancelled.stoppingPreview, false);
  assert.equal(cancelled.previewing, false);

  const arrived = run([
    { type: "previewStarted" },
    { type: "previewStopRequested" },
    { type: "previewArrived", preview: preview(6) },
  ]);
  assert.equal(arrived.stoppingPreview, false, "a discovery that beat the kill still lands");
  assert.equal(canReview(arrived), true);

  const idle = run([{ type: "previewStopRequested" }]);
  assert.equal(idle.stoppingPreview, false, "nothing to stop");

  const restarted = run([{ type: "previewStarted" }], {
    ...INITIAL_CLEANUP,
    stoppingPreview: true,
  });
  assert.equal(restarted.stoppingPreview, false);
});

test("a failed preview says so without claiming a run happened", () => {
  const failed = run([
    { type: "previewStarted" },
    { type: "previewFailed", message: "mo is not installed" },
  ]);

  assert.equal(failed.previewing, false);
  assert.equal(failed.previewError, "mo is not installed");
  assert.equal(failed.result, null);
  assert.equal(canReview(failed), false);
});

/** Only a clean finish reads as success. Everything else removed an unknown amount. */
test("every ending has its own wording and only one is good news", () => {
  assert.equal(outcomeLabel("finished"), "Finished");
  assert.equal(outcomeLabel("partial"), "Partial");
  assert.equal(outcomeLabel("cancelled"), "Stopped");
  assert.equal(outcomeLabel("failed"), "Failed");

  assert.equal(outcomeTone("finished"), "success");
  for (const completion of ["partial", "cancelled", "failed"] as const) {
    assert.equal(outcomeTone(completion), "warning", completion);
  }
});
