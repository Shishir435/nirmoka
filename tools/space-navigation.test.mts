import assert from "node:assert/strict";
import test from "node:test";

import { parentIdForScan } from "../apps/desktop/src/pages/space-navigation.ts";

test("directory navigation is retained only for the scan that created it", () => {
  const location = { scanId: 41, parentId: 7 };

  assert.equal(parentIdForScan(location, 41), 7);
  assert.equal(parentIdForScan(location, 42), null);
  assert.equal(parentIdForScan(location, null), null);
});
