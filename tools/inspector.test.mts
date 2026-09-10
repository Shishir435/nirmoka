import assert from "node:assert/strict";
import test from "node:test";

import {
  componentShare,
  isLowerBound,
  reclaimableFor,
} from "../apps/desktop/src/lib/engine/inspector.ts";

const HOME = "/Users/example";

const path = (p: string, complete = true) => ({
  path: p,
  totalBytes: 1000,
  complete,
  source: "scan" as const,
});

const footprint = (overrides: Record<string, unknown> = {}) => ({
  scanId: 1,
  nodeId: 5,
  name: "Example",
  path: "/Applications/Example.app",
  bundleId: "com.example.desktop",
  totalBytes: 4000,
  relatedBytes: 0,
  unmeasuredPaths: 0,
  lastUsedMs: null,
  components: [
    {
      label: "Application",
      totalBytes: 1000,
      complete: true,
      certain: true,
      paths: [path("/Applications/Example.app")],
    },
    {
      label: "Caches",
      totalBytes: 3000,
      complete: true,
      certain: true,
      paths: [path("/Users/example/Library/Caches/com.example.desktop")],
    },
    {
      label: "Possibly related",
      totalBytes: 9000,
      complete: true,
      certain: false,
      paths: [path("/Users/example/Library/Application Support/Example")],
    },
  ],
  ...overrides,
});

const preview = (items: string[]) => ({
  backend: "mole",
  backendInsteadOf: null,
  backendVersion: "1.48.1",
  generatedAt: "2026-08-20",
  categories: [
    {
      name: "App caches",
      items: items.map((p) => ({ path: p, reportedSize: "1MB", itemCount: 3 })),
    },
  ],
  potentialCleanup: "1MB",
  totalItems: items.length,
  systemScope: "userOnly" as const,
  warnings: [],
});

test("Mole's rows are attributed by path, under Mole's own category name", () => {
  const found = reclaimableFor(
    footprint(),
    preview(["/Users/example/Library/Caches/com.example.desktop"]),
    HOME,
  );

  assert.equal(found.length, 1);
  assert.equal(found[0]?.category, "App caches", "the backend's word, not ours");
});

test("a tilde path and an expanded one are the same directory", () => {
  // Mole abbreviates home and the footprint does not. Comparing them raw finds
  // nothing, which is the failure this normalisation exists for.
  const found = reclaimableFor(
    footprint(),
    preview(["~/Library/Caches/com.example.desktop"]),
    HOME,
  );

  assert.equal(found.length, 1);
});

test("a trailing wildcard still names the directory it describes", () => {
  const found = reclaimableFor(
    footprint(),
    preview(["~/Library/Caches/com.example.desktop/*"]),
    HOME,
  );

  assert.equal(found.length, 1);
});

test("a row inside the footprint counts; a neighbour's does not", () => {
  const found = reclaimableFor(
    footprint(),
    preview([
      "/Users/example/Library/Caches/com.example.desktop/blobs",
      "/Users/example/Library/Caches/com.example.other",
      // A prefix of the name is not a prefix of the path: `…desktop2` is a
      // different application and must not be swept in.
      "/Users/example/Library/Caches/com.example.desktop2",
    ]),
    HOME,
  );

  assert.deepEqual(
    found.map((r) => r.item.path),
    ["/Users/example/Library/Caches/com.example.desktop/blobs"],
  );
});

test("a guessed component never contributes a removable row", () => {
  // ADR 0028: vendor-named directories are matched by name and excluded from
  // the total. Hanging a removal off that guess is exactly what must not happen.
  const found = reclaimableFor(
    footprint(),
    preview(["/Users/example/Library/Application Support/Example"]),
    HOME,
  );

  assert.deepEqual(found, []);
});

test("no preview is no rows, not an error", () => {
  assert.deepEqual(reclaimableFor(footprint(), null, HOME), []);
  assert.deepEqual(reclaimableFor(footprint(), preview([]), HOME), []);
});

test("a component's share is of the footprint, and a guess has none", () => {
  const f = footprint();
  const [application, caches, related] = f.components;

  assert.equal(componentShare(application!, f), 0.25);
  assert.equal(componentShare(caches!, f), 0.75);
  // Excluded from the total, so a share of it would exceed 1.
  assert.equal(componentShare(related!, f), 0);
});

test("a footprint with an unread part says the total is a bound", () => {
  assert.equal(isLowerBound(footprint()), false);
  assert.equal(isLowerBound(footprint({ unmeasuredPaths: 1 })), true);

  const partial = footprint();
  partial.components[1]!.complete = false;
  assert.equal(isLowerBound(partial), true);
});
