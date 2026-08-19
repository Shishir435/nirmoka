import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_LOCATION,
  hashForLocation,
  locationFromHash,
} from "../apps/desktop/src/lib/engine/route.ts";

test("a bare route carries no view, and bare storage is the dashboard", () => {
  assert.deepEqual(locationFromHash("#/clean"), { route: "clean", view: null });
  assert.deepEqual(locationFromHash("#/activity"), { route: "activity", view: null });
  // The one destination: no view means the dashboard, not a browser default.
  assert.deepEqual(locationFromHash("#/storage"), DEFAULT_LOCATION);
  assert.equal(DEFAULT_LOCATION.view, null);
});

test("a storage view survives the hash it was written into", () => {
  assert.deepEqual(locationFromHash("#/storage/developer"), {
    route: "storage",
    view: "developer",
  });
  assert.deepEqual(locationFromHash("#/storage/applications"), {
    route: "storage",
    view: "applications",
  });
});

test("every retired hash lands on the page that absorbed it", () => {
  // Overview's content is the dashboard now, and system status sat on it, so
  // both land there rather than in the browser they passed through under 0026.
  assert.deepEqual(locationFromHash("#/overview"), { route: "storage", view: null });
  assert.deepEqual(locationFromHash("#/space"), { route: "storage", view: "folders" });
  assert.deepEqual(locationFromHash("#/status"), { route: "storage", view: null });
  assert.deepEqual(locationFromHash("#/developer"), { route: "storage", view: "developer" });
  assert.deepEqual(locationFromHash("#/applications"), {
    route: "storage",
    view: "applications",
  });
});

test("onboarding is answered as itself, because it replaces the shell", () => {
  assert.equal(locationFromHash("#/onboarding"), "onboarding");
  assert.equal(locationFromHash("#/onboarding/backends"), "onboarding");
});

test("nothing recognisable is the default rather than an error", () => {
  assert.deepEqual(locationFromHash(""), DEFAULT_LOCATION);
  assert.deepEqual(locationFromHash("#"), DEFAULT_LOCATION);
  assert.deepEqual(locationFromHash("#/"), DEFAULT_LOCATION);
  assert.deepEqual(locationFromHash("#/nowhere"), DEFAULT_LOCATION);
});

test("an unknown view on a known route is no view, not a broken page", () => {
  assert.deepEqual(locationFromHash("#/storage/nowhere"), DEFAULT_LOCATION);
  assert.deepEqual(locationFromHash("#/clean/developer"), { route: "clean", view: "developer" });
});

test("a hash written from a location reads back as that location", () => {
  for (const location of [
    { route: "storage", view: null },
    { route: "storage", view: "folders" },
    { route: "storage", view: "developer" },
    { route: "storage", view: "applications" },
    { route: "clean", view: null },
    { route: "activity", view: null },
    { route: "help", view: null },
  ] as const) {
    assert.deepEqual(locationFromHash(hashForLocation(location)), location);
  }
});

test("the dashboard and the browser are different links", () => {
  // They were the same place under ADR 0026, which is exactly what changed:
  // #/storage is now the dashboard and the browser is a screen below it.
  assert.equal(hashForLocation({ route: "storage", view: null }), "#/storage");
  assert.equal(hashForLocation({ route: "storage", view: "folders" }), "#/storage/folders");
  assert.equal(hashForLocation({ route: "storage", view: "developer" }), "#/storage/developer");
  // A view outside storage names nothing, so it stays out of the hash.
  assert.equal(hashForLocation({ route: "clean", view: "developer" }), "#/clean");
});
