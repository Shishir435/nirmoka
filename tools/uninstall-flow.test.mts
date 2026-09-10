import assert from "node:assert/strict";
import test from "node:test";

import {
  canUninstall,
  INITIAL_UNINSTALL,
  planCounts,
  reduceUninstall,
  uninstallOutcomeMessage,
  type UninstallEvent,
  type UninstallState,
  groupOf,
  groupPlan,
  survivingItems,
} from "../apps/desktop/src/lib/engine/uninstall-flow.ts";

const item = (displayPath: string, scope = "removed") =>
  ({ displayPath, reportedSize: null, scope }) as never;

const preview = (name = "Example", items = [item("/Applications/Example.app")]) =>
  ({
    backend: "mole",
    backendVersion: "1.48.1",
    requested: [name.toLowerCase()],
    reportedTotal: "83.4MB",
    totalItems: items.length,
    hasReviewOnlyItems: false,
    warnings: [],
    notes: [],
    transcript: "",
    apps: [{ name, homebrewCask: false, reportedSize: "83.4MB", items }],
  }) as never;

const preparation = (token = 7) =>
  ({
    confirmationToken: token,
    backend: "mole",
    backendVersion: "1.48.1",
    applications: ["Example"],
    reportedTotal: "83.4MB",
    totalItems: 1,
    hasReviewOnlyItems: false,
    warnings: [],
    expiresInSeconds: 300,
    requiresConfirmation: true,
    warning: "Example and the files listed above will be moved to the Trash by mole.",
  }) as never;

const operation = (
  completion = "finished",
  removed = ["Example"],
  failed: string[] = [],
  logError: string | null = null,
) =>
  ({
    id: 3,
    backend: "mole",
    backendVersion: "1.48.1",
    reviewedApplications: ["Example"],
    reviewedItems: 1,
    reviewedTotal: "83.4MB",
    completion,
    removed,
    failed,
    reportedFreed: "83.4MB",
    warnings: [],
    executedAtMs: 1_760_000_000_000,
    logError,
  }) as never;

const run = (events: UninstallEvent[], from: UninstallState = INITIAL_UNINSTALL) =>
  events.reduce(reduceUninstall, from);

test("a plan cannot be confirmed before it has been shown", () => {
  const reviewing = run([{ type: "reviewStarted", requestId: 1, name: "example" }]);
  assert.equal(reviewing.preview, null);
  assert.equal(reviewing.preparation, null);

  const shown = run([{ type: "reviewed", requestId: 1, preview: preview() }], reviewing);
  assert.notEqual(shown.preview, null);
  // Still nothing to confirm with: the token is a second round trip.
  assert.equal(shown.preparation, null);

  const approved = run([{ type: "prepared", requestId: 1, preparation: preparation() }], shown);
  assert.equal(approved.preparation?.confirmationToken, 7);
  // The plan stays on screen beside its confirmation.
  assert.notEqual(approved.preview, null);
});

test("starting a run drops the token and the plan, so a second click resubmits nothing", () => {
  const state = run([
    { type: "reviewStarted", requestId: 1, name: "example" },
    { type: "reviewed", requestId: 1, preview: preview() },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 2 },
  ]);

  assert.equal(state.preparation, null);
  assert.equal(state.preview, null);
  assert.equal(state.running, true);
});

test("a late reply from an abandoned review does not open a sheet", () => {
  const state = run([
    { type: "reviewStarted", requestId: 1, name: "first" },
    { type: "reviewStarted", requestId: 2, name: "second" },
    // The first request finally answers. It names an application the user has
    // moved on from, and showing it would describe the wrong removal.
    { type: "reviewed", requestId: 1, preview: preview("First") },
  ]);

  assert.equal(state.preview, null);
  assert.equal(state.activeName, "second");
  assert.equal(state.reviewing, true);
});

test("a late failure from an abandoned review does not clear the live one", () => {
  const state = run([
    { type: "reviewStarted", requestId: 1, name: "first" },
    { type: "reviewStarted", requestId: 2, name: "second" },
    { type: "reviewFailed", requestId: 1, message: "gone" },
  ]);

  assert.equal(state.reviewing, true);
  assert.equal(state.error, null);
  assert.equal(state.activeName, "second");
});

test("only what the backend reported as removed is marked", () => {
  // The user asked about two, and the backend removed one. Marking both would
  // claim a removal that did not happen.
  const state = run([
    { type: "reviewStarted", requestId: 1, name: "example" },
    { type: "reviewed", requestId: 1, preview: preview() },
    { type: "prepared", requestId: 1, preparation: preparation() },
    { type: "runStarted", requestId: 2 },
    {
      type: "removed",
      requestId: 2,
      operation: operation("partial", ["Example"], ["Other is still running"]),
    },
  ]);

  assert.deepEqual(state.removedNames, ["Example"]);
  assert.equal(state.running, false);
});

test("a run that never started marks nothing", () => {
  const state = run([
    { type: "runStarted", requestId: 1 },
    { type: "runFailed", requestId: 1, message: "this confirmation was already used" },
  ]);

  assert.deepEqual(state.removedNames, []);
  assert.equal(state.error, "this confirmation was already used");
  assert.equal(state.running, false);
});

test("a stale run reply neither re-enables the buttons nor marks a row", () => {
  const state = run([
    { type: "runStarted", requestId: 1 },
    { type: "runStarted", requestId: 2 },
    { type: "removed", requestId: 1, operation: operation() },
  ]);

  assert.equal(state.running, true);
  assert.deepEqual(state.removedNames, []);
});

test("reloading the inventory keeps what was already removed", () => {
  const removed = run([
    { type: "runStarted", requestId: 1 },
    { type: "removed", requestId: 1, operation: operation() },
  ]);
  const state = run([{ type: "inventoryReloaded" }], removed);

  assert.deepEqual(state.removedNames, ["Example"]);
  assert.equal(state.preview, null);
});

test("a run in flight survives an inventory reload", () => {
  // It cannot be abandoned: the backend is moving files, and its result still
  // belongs in the journal.
  const state = run([{ type: "runStarted", requestId: 1 }, { type: "inventoryReloaded" }]);

  assert.equal(state.running, true);
  assert.equal(state.runningRequestId, 1);
});

test("an application already removed cannot be reviewed again", () => {
  const state = run([
    { type: "runStarted", requestId: 1 },
    { type: "removed", requestId: 1, operation: operation() },
  ]);

  assert.equal(canUninstall(state, "Example"), false);
  assert.equal(canUninstall(state, "Other"), true);
  assert.equal(canUninstall(state, null), false);
  // Not while a review or a run is in flight, and not while a plan is open.
  assert.equal(canUninstall({ ...INITIAL_UNINSTALL, reviewing: true }, "Other"), false);
  assert.equal(canUninstall({ ...INITIAL_UNINSTALL, running: true }, "Other"), false);
  assert.equal(canUninstall({ ...INITIAL_UNINSTALL, preview: preview() }, "Other"), false);
});

test("paths the backend will leave behind are counted apart from the ones it removes", () => {
  const counts = planCounts(
    preview("Example", [
      item("/Applications/Example.app"),
      item("/Library/LaunchDaemons/com.example.plist", "system"),
      item("/Library/Preferences/com.example.plist", "reviewOnly"),
    ]),
  );

  // A single total would say three things will be removed. Two will.
  assert.equal(counts.removed, 2);
  assert.equal(counts.reviewOnly, 1);
});

test("the outcome message reports the backend's own completion", () => {
  assert.match(uninstallOutcomeMessage(operation()), /Example moved to the Trash/u);
  assert.match(
    uninstallOutcomeMessage(operation("partial", ["Example"], ["Other is still running"])),
    /could not remove: Other is still running/u,
  );
  assert.match(
    uninstallOutcomeMessage(operation("cancelled", [], [])),
    /stopped part way through/u,
  );
  assert.match(uninstallOutcomeMessage(operation("failed", [], [])), /did not finish/u);
});

test("a move that happened but was not logged says both", () => {
  const message = uninstallOutcomeMessage(operation("finished", ["Example"], [], "disk full"));

  assert.match(message, /moved to the Trash/u);
  assert.match(message, /could not be added to the operation log: disk full/u);
});

test("a plan is grouped by where its paths sit, in the footprint's vocabulary", () => {
  // The same names attribution.rs uses, so the screen that says what an
  // application costs and the screen that says what removing it touches
  // describe the same places the same way.
  const items = [
    item("~/Library/Caches/com.example.desktop"),
    item("/Applications/Example.app"),
    item("~/Library/Preferences/com.example.desktop.plist"),
    item("~/Library/Containers/com.example.desktop"),
    item("~/Library/Application Scripts/com.example.desktop"),
    item("~/Library/Application Support/com.example.desktop"),
    item("~/Library/Logs/com.example.desktop"),
    item("~/.example-rc"),
  ];

  assert.deepEqual(
    groupPlan(items).map((group) => [group.label, group.items.length]),
    [
      ["Application", 1],
      ["Containers", 2],
      ["Application Support", 1],
      ["Caches", 1],
      ["Logs", 1],
      ["Preferences", 1],
      ["Other", 1],
    ],
  );
});

test("groups nothing fell into are absent, not empty", () => {
  assert.deepEqual(
    groupPlan([item("/Applications/Example.app")]).map((group) => group.label),
    ["Application"],
  );
  assert.deepEqual(groupPlan([]), []);
});

test("a saved-state file is preferences, not something uncategorised", () => {
  assert.equal(
    groupOf("~/Library/Saved Application State/com.example.desktop.savedState"),
    "Preferences",
  );
  assert.equal(groupOf("~/Library/WebKit/com.example.desktop"), "Caches");
  assert.equal(groupOf("~/Library/HTTPStorages/com.example.desktop"), "Caches");
});

test("what the backend will not remove is separated from what it will", () => {
  // ADR 0029: this stands where the design puts a keep-user-data choice, and
  // it has to be the backend's answer rather than an option we invented.
  const items = [
    item("/Applications/Example.app"),
    { ...item("/Library/LaunchDaemons/com.example.plist"), scope: "system" as const },
    { ...item("~/Library/Group Containers/example"), scope: "reviewOnly" as const },
  ];

  assert.deepEqual(
    survivingItems(items).map((i) => i.displayPath),
    ["/Library/LaunchDaemons/com.example.plist", "~/Library/Group Containers/example"],
  );
});
