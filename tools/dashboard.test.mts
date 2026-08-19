import assert from "node:assert/strict";
import test from "node:test";

import {
  applyFootprint,
  applyIcon,
  barTotal,
  barVolume,
  isBundle,
  openTarget,
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

const consumer = (
  id: number,
  name: string,
  totalBytes: number,
  path = `/x/${name}`,
  extra: { isDir?: boolean; parentId?: number | null } = {},
) => ({
  id,
  name,
  path,
  totalBytes,
  sizeIsPartial: false,
  isDir: extra.isDir ?? true,
  parentId: extra.parentId ?? null,
});

const ranked = (c: ReturnType<typeof consumer>) => ({
  consumer: c,
  category: "other" as const,
  measure: "size" as const,
  icon: null,
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
  const slices = usageSlices(
    breakdown({ volume: true }),
    display,
    "free-colour",
    "unscanned-colour",
  );

  assert.deepEqual(slices.at(-1), { key: "free", label: "Free", bytes: 600, color: "free-colour" });
  assert.equal(barTotal(breakdown({ volume: true })), 1000);
});

test("space in use that was not scanned is its own slice, not bare track", () => {
  // 400 in use, 300 of it scanned. The missing 100 is occupied, and leaving it
  // as track would draw it in the same colour as free space.
  const slices = usageSlices(
    breakdown({ volume: true }),
    display,
    "free-colour",
    "unscanned-colour",
  );
  const unscanned = slices.find((slice) => slice.key === "unscanned");

  assert.deepEqual(unscanned, {
    key: "unscanned",
    label: "Not scanned",
    bytes: 100,
    color: "unscanned-colour",
  });
  // Nothing shows through: the slices are exactly the volume.
  assert.equal(
    slices.reduce((sum, slice) => sum + slice.bytes, 0),
    barTotal(breakdown({ volume: true })),
  );
});

test("a scan covering the whole volume has no unscanned slice", () => {
  const whole = {
    ...breakdown({ volume: true }),
    scannedBytes: 400,
  };

  assert.ok(
    !usageSlices(whole, display, "free-colour", "unscanned-colour").some(
      (s) => s.key === "unscanned",
    ),
  );
});

test("without capacity the bar is the scan alone", () => {
  const slices = usageSlices(breakdown(), display, "free-colour", "unscanned-colour");

  assert.equal(slices.length, 5);
  assert.ok(!slices.some((slice) => slice.key === "free"));
  // The widths then divide the scan, so they still fill the bar.
  assert.equal(barTotal(breakdown()), 300);
});

test("a scan larger than the volume reports drops the volume frame", () => {
  // The window scans one filesystem, so this is measurements disagreeing — a
  // snapshot the walk counted and df did not, or apparent sizes. Keeping the
  // volume would draw categories plus free space past the end of the bar.
  const disagreeing = {
    ...breakdown({ volume: true }),
    scannedBytes: 500,
  };

  assert.equal(barVolume(disagreeing), null);
  assert.equal(barTotal(disagreeing), 500, "the bar is the scan");

  const slices = usageSlices(disagreeing, display, "free-colour", "unscanned-colour");
  assert.ok(!slices.some((slice) => slice.key === "free"));
  assert.ok(!slices.some((slice) => slice.key === "unscanned"));
  assert.equal(
    slices.reduce((sum, slice) => sum + slice.bytes, 0),
    300,
    "the slices are the categories, nothing invented to fill the bar",
  );
});

test("a scan that exactly fills the volume keeps it as the frame", () => {
  const exact = { ...breakdown({ volume: true }), scannedBytes: 400 };
  assert.notEqual(barVolume(exact), null);
  assert.equal(barTotal(exact), 1000);
});

test("every category keeps a slice, including the empty ones", () => {
  const slices = usageSlices(breakdown(), display, "free-colour", "unscanned-colour");

  assert.deepEqual(
    slices.map((slice) => slice.key),
    ["apps", "personalFiles", "development", "system", "other"],
  );
});

test("the biggest users mix applications and directories", () => {
  const rows = rankConsumers(breakdown());

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
  const rows = rankConsumers(breakdown());
  // Docker's bundle is 100 and its real footprint is far larger, which is the
  // whole reason the Inspector exists. It has to move to the top.
  const updated = applyFootprint(rows, 1, 4_700);

  assert.deepEqual(updated[0]?.consumer.name, "Docker.app");
  assert.equal(updated[0]?.consumer.totalBytes, 4_700);
  assert.equal(updated[0]?.measure, "footprint");
  assert.equal(updated[1]?.consumer.name, "Downloads");
});

test("a footprint for a row that is gone changes nothing", () => {
  const rows = rankConsumers(breakdown());

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
    rankConsumers(tied).map((row) => row.consumer.path),
    ["/a", "/b"],
  );
});

test("an icon decorates a row without reordering the list", () => {
  const rows = rankConsumers(breakdown());
  const decorated = applyIcon(rows, 1, "data:image/png;base64,AA");

  assert.deepEqual(
    decorated.map((row) => row.consumer.name),
    rows.map((row) => row.consumer.name),
  );
  assert.equal(decorated.find((row) => row.consumer.id === 1)?.icon, "data:image/png;base64,AA");
  // A bundle with no readable icon keeps its fallback rather than breaking.
  assert.equal(applyIcon(rows, 1, null).find((row) => row.consumer.id === 1)?.icon, null);
});

test("a small bundle stays a candidate so its footprint can promote it", () => {
  // Docker's bundle is the smallest thing here and its footprint is the
  // largest. Trimming to the visible rows before the footprint arrived would
  // drop it, and the biggest thing on the disk would never be listed.
  const many = {
    ...breakdown(),
    categories: [
      {
        category: "apps" as const,
        totalBytes: 10,
        share: 0.1,
        consumers: [consumer(9, "Docker.app", 10, "/Applications/Docker.app")],
      },
      {
        category: "personalFiles" as const,
        totalBytes: 600,
        share: 0.9,
        consumers: [
          consumer(1, "A", 600, "/a"),
          consumer(2, "B", 500, "/b"),
          consumer(3, "C", 400, "/c"),
          consumer(4, "D", 300, "/d"),
          consumer(5, "E", 200, "/e"),
          consumer(6, "F", 100, "/f"),
        ],
      },
    ],
  };

  const rows = rankConsumers(many);
  assert.equal(rows.at(-1)?.consumer.name, "Docker.app", "last by bundle size");
  assert.equal(rows.length, 7, "nothing is dropped before the footprints land");

  const promoted = applyFootprint(rows, 9, 4_700);
  assert.equal(promoted[0]?.consumer.name, "Docker.app");
  // And it is inside the visible six, which is the point.
  assert.ok(promoted.slice(0, 6).some((row) => row.consumer.id === 9));
});

test("a file opens the directory holding it, not itself", () => {
  const file = consumer(7, "big.iso", 900, "/users/example/big.iso", {
    isDir: false,
    parentId: null,
  });
  const nested = consumer(8, "clip.mov", 900, "/users/example/Movies/clip.mov", {
    isDir: false,
    parentId: 4,
  });
  const directory = consumer(9, "Movies", 900, "/users/example/Movies");
  // Browsing "into" a file would list an empty directory.
  assert.equal(openTarget(ranked(file)), null, "its parent is the scan root");
  assert.equal(openTarget(ranked(nested)), 4);
  assert.equal(openTarget(ranked(directory)), 9);
});

test("only a .app is measured as an application", () => {
  assert.equal(isBundle(consumer(1, "Docker.app", 1)), true);
  assert.equal(isBundle(consumer(1, "Docker.APP", 1)), true);
  assert.equal(isBundle(consumer(1, "Downloads", 1)), false);
  assert.equal(isBundle(consumer(1, "notes.app.txt", 1)), false);
});

test("nothing to show is an empty list, not a crash", () => {
  assert.deepEqual(rankConsumers(null), []);

  const bare = { ...breakdown(), categories: [] };
  assert.deepEqual(rankConsumers(bare), []);
  assert.deepEqual(usageSlices(bare, display, "free-colour", "unscanned-colour"), []);
});
