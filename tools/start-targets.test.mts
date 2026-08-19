import assert from "node:assert/strict";
import test from "node:test";

import {
  isNearlyFull,
  START_TARGETS,
  usedFraction,
} from "../apps/desktop/src/lib/engine/start-targets.ts";

test("a normal volume reports the used share of its capacity", () => {
  // The real numbers from a 494 GB Mac: df reports 238 used and 230 free, which
  // do not add up to the total.
  assert.equal(
    Math.round(usedFraction({ totalBytes: 494_384_795_648, usedBytes: 238_526_865_408 }) * 100),
    48,
  );
  assert.equal(usedFraction({ totalBytes: 100, usedBytes: 25 }), 0.25);
});

test("an empty or unreadable volume is not full", () => {
  assert.equal(usedFraction({ totalBytes: 0, usedBytes: 0 }), 0);
  assert.equal(usedFraction({ totalBytes: 0, usedBytes: 500 }), 0);
  assert.equal(usedFraction({ totalBytes: Number.NaN, usedBytes: 10 }), 0);
  assert.equal(usedFraction({ totalBytes: 100, usedBytes: Number.NaN }), 0);
  assert.equal(usedFraction({ totalBytes: 100, usedBytes: -5 }), 0);
});

test("used above total clamps rather than overflowing the bar", () => {
  assert.equal(usedFraction({ totalBytes: 100, usedBytes: 140 }), 1);
});

test("nearly full starts at ninety percent", () => {
  assert.equal(isNearlyFull({ totalBytes: 100, usedBytes: 89 }), false);
  assert.equal(isNearlyFull({ totalBytes: 100, usedBytes: 90 }), true);
  assert.equal(isNearlyFull({ totalBytes: 100, usedBytes: 99 }), true);
  // An unreadable volume must not raise an alarm it has no basis for.
  assert.equal(isNearlyFull({ totalBytes: 0, usedBytes: 0 }), false);
});

test("every start target is distinct and scannable", () => {
  assert.equal(new Set(START_TARGETS.map((target) => target.id)).size, START_TARGETS.length);
  assert.equal(new Set(START_TARGETS.map((target) => target.path)).size, START_TARGETS.length);
  for (const target of START_TARGETS) {
    assert.ok(target.path.length > 0, `${target.id} needs a path`);
    assert.ok(target.label.length > 0, `${target.id} needs a label`);
    // A relative path would resolve against the process working directory,
    // which is wherever the bundle was launched from.
    assert.ok(
      target.path.startsWith("~") || target.path.startsWith("/"),
      `${target.id} must be absolute or home-relative, got ${target.path}`,
    );
  }
  assert.equal(START_TARGETS[0]?.path, "~", "home is the default and comes first");
});
