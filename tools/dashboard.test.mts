import assert from "node:assert/strict";
import test from "node:test";

import {
  applyFootprint,
  applyIcon,
  barTotal,
  isBundle,
  rankConsumers,
  usageSlices,
} from "../apps/desktop/src/lib/engine/dashboard.ts";

const display = {
  apps: { label: "Apps", color: "a" },
  personalFiles: { label: "Personal Files", color: "p" },
  development: { label: "Development", color: "d" },
  system: { label: "System", color: "s" },
  other: { label: "Other", color: "o" },
};

const consumer = (id: number, name: string, totalBytes: number, path = `/x/${name}`) => ({
  id,
  name,
  path,
  totalBytes,
  sizeIsPartial: false,
});

const breakdown = (options: { volume?: boolean } = {}) => ({
  scanId: 1,
  rootPath: "/users/example",
  scannedBytes: 300,
  volume: options.volume
    ? { name: "Macintosh HD", mountPoint: "/", totalBytes: 1000, usedBytes: 400, freeBytes: 600 }
    : null,
  categories: [
    {
      category: "apps" as const,
      totalBytes: 100,
      share: 1 / 3,
      consumers: [consumer(1, "Docker.app", 100)],
    },
    {
      category: "personalFiles" as const,
      totalBytes: 150,
      share: 0.5,
      consumers: [consumer(2, "Downloads", 150)],
    },
    {
      category: "development" as const,
      totalBytes: 50,
      share: 1 / 6,
      consumers: [consumer(3, "node_modules", 50)],
    },
    { category: "system" as const, totalBytes: 0, share: 0, consumers: [] },
    { category: "other" as const, totalBytes: 0, share: 0, consumers: [] },
  ],
});

test("free space joins the bar so it describes the volume, not the scan", () => {
  const slices = usageSlices(breakdown({ volume: true }), display, "free-colour");

  assert.equal(slices.length, 6);
  assert.deepEqual(slices.at(-1), { key: "free", label: "Free", bytes: 600, color: "free-colour" });
  assert.equal(barTotal(breakdown({ volume: true })), 1000);
});

test("without capacity the bar is the scan alone", () => {
  const slices = usageSlices(breakdown(), display, "free-colour");

  assert.equal(slices.length, 5);
  assert.ok(!slices.some((slice) => slice.key === "free"));
  // The widths then divide the scan, so they still fill the bar.
  assert.equal(barTotal(breakdown()), 300);
});

test("every category keeps a slice, including the empty ones", () => {
  const slices = usageSlices(breakdown(), display, "free-colour");

  assert.deepEqual(
    slices.map((slice) => slice.key),
    ["apps", "personalFiles", "development", "system", "other"],
  );
});

test("the biggest users mix applications and directories", () => {
  const rows = rankConsumers(breakdown(), 6);

  assert.deepEqual(
    rows.map((row) => [row.consumer.name, row.measure]),
    [
      ["Downloads", "size"],
      ["Docker.app", "bundle"],
      ["node_modules", "size"],
    ],
  );
});

test("a footprint replaces the bundle size and re-sorts the list", () => {
  const rows = rankConsumers(breakdown(), 6);
  // Docker's bundle is 100 and its real footprint is far larger, which is the
  // whole reason the Inspector exists. It has to move to the top.
  const updated = applyFootprint(rows, 1, 4_700);

  assert.deepEqual(updated[0]?.consumer.name, "Docker.app");
  assert.equal(updated[0]?.consumer.totalBytes, 4_700);
  assert.equal(updated[0]?.measure, "footprint");
  assert.equal(updated[1]?.consumer.name, "Downloads");
});

test("a footprint for a row that is gone changes nothing", () => {
  const rows = rankConsumers(breakdown(), 6);

  assert.deepEqual(applyFootprint(rows, 999, 1), rows);
});

test("ties break on the path so rows do not swap between renders", () => {
  const tied = {
    ...breakdown(),
    categories: [
      {
        category: "apps" as const,
        totalBytes: 20,
        share: 1,
        consumers: [consumer(1, "B", 10, "/b"), consumer(2, "A", 10, "/a")],
      },
    ],
  };

  assert.deepEqual(
    rankConsumers(tied, 6).map((row) => row.consumer.path),
    ["/a", "/b"],
  );
});

test("an icon decorates a row without reordering the list", () => {
  const rows = rankConsumers(breakdown(), 6);
  const decorated = applyIcon(rows, 1, "data:image/png;base64,AA");

  assert.deepEqual(
    decorated.map((row) => row.consumer.name),
    rows.map((row) => row.consumer.name),
  );
  assert.equal(decorated.find((row) => row.consumer.id === 1)?.icon, "data:image/png;base64,AA");
  // A bundle with no readable icon keeps its fallback rather than breaking.
  assert.equal(applyIcon(rows, 1, null).find((row) => row.consumer.id === 1)?.icon, null);
});

test("only a .app is measured as an application", () => {
  assert.equal(isBundle(consumer(1, "Docker.app", 1)), true);
  assert.equal(isBundle(consumer(1, "Docker.APP", 1)), true);
  assert.equal(isBundle(consumer(1, "Downloads", 1)), false);
  assert.equal(isBundle(consumer(1, "notes.app.txt", 1)), false);
});

test("a limit of nothing to show is an empty list, not a crash", () => {
  assert.deepEqual(rankConsumers(null, 6), []);
  assert.deepEqual(rankConsumers(breakdown(), 0), []);
});
