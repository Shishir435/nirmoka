import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_LOCATION,
  hashForLocation,
  locationFromHash,
} from "../apps/desktop/src/lib/engine/route.ts";

test("a bare route resolves to itself and the default view", () => {
  assert.deepEqual(locationFromHash("#/clean"), { route: "clean", view: "folders" });
  assert.deepEqual(locationFromHash("#/activity"), { route: "activity", view: "folders" });
  assert.deepEqual(locationFromHash("#/storage"), DEFAULT_LOCATION);
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
  assert.deepEqual(locationFromHash("#/overview"), { route: "storage", view: "folders" });
  assert.deepEqual(locationFromHash("#/space"), { route: "storage", view: "folders" });
  assert.deepEqual(locationFromHash("#/status"), { route: "storage", view: "folders" });
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

test("an unknown view on a known route is the default view, not a broken page", () => {
  assert.deepEqual(locationFromHash("#/storage/nowhere"), DEFAULT_LOCATION);
  assert.deepEqual(locationFromHash("#/clean/developer"), { route: "clean", view: "developer" });
});

test("a hash written from a location reads back as that location", () => {
  for (const location of [
    { route: "storage", view: "folders" },
    { route: "storage", view: "developer" },
    { route: "storage", view: "applications" },
    { route: "clean", view: "folders" },
    { route: "activity", view: "folders" },
    { route: "help", view: "folders" },
  ] as const) {
    assert.deepEqual(locationFromHash(hashForLocation(location)), location);
  }
});

test("the default view is left out of the hash, so one place has one link", () => {
  assert.equal(hashForLocation({ route: "storage", view: "folders" }), "#/storage");
  assert.equal(hashForLocation({ route: "storage", view: "developer" }), "#/storage/developer");
  assert.equal(hashForLocation({ route: "clean", view: "developer" }), "#/clean");
});
