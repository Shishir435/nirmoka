import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_LOCATION,
  firstLocation,
  hashForLocation,
  locationFromHash,
} from "../apps/desktop/src/lib/engine/route.ts";

test("a bare route carries no view, and bare storage is the dashboard", () => {
  assert.deepEqual(locationFromHash("#/clean"), { route: "clean", view: null, inspect: null });
  assert.deepEqual(locationFromHash("#/activity"), {
    route: "activity",
    view: null,
    inspect: null,
  });
  // The one destination: no view means the dashboard, not a browser default.
  assert.deepEqual(locationFromHash("#/storage"), DEFAULT_LOCATION);
  assert.equal(DEFAULT_LOCATION.view, null);
});

test("a storage view survives the hash it was written into", () => {
  assert.deepEqual(locationFromHash("#/storage/developer"), {
    route: "storage",
    view: "developer",
    inspect: null,
  });
  assert.deepEqual(locationFromHash("#/storage/applications"), {
    route: "storage",
    view: "applications",
    inspect: null,
  });
});

test("every retired hash lands on the page that absorbed it", () => {
  // Overview's content is the dashboard now, and system status sat on it, so
  // both land there rather than in the browser they passed through under 0026.
  assert.deepEqual(locationFromHash("#/overview"), { route: "storage", view: null, inspect: null });
  assert.deepEqual(locationFromHash("#/space"), {
    route: "storage",
    view: "folders",
    inspect: null,
  });
  assert.deepEqual(locationFromHash("#/status"), { route: "storage", view: null, inspect: null });
  assert.deepEqual(locationFromHash("#/developer"), {
    route: "storage",
    view: "developer",
    inspect: null,
  });
  assert.deepEqual(locationFromHash("#/applications"), {
    route: "storage",
    view: "applications",
    inspect: null,
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
  assert.deepEqual(locationFromHash("#/clean/developer"), {
    route: "clean",
    view: "developer",
    inspect: null,
  });
});

test("a hash written from a location reads back as that location", () => {
  for (const location of [
    { route: "storage", view: null, inspect: null },
    { route: "storage", view: "folders", inspect: null },
    { route: "storage", view: "developer", inspect: null },
    { route: "storage", view: "applications", inspect: null },
    { route: "clean", view: null, inspect: null },
    { route: "activity", view: null, inspect: null },
    { route: "help", view: null, inspect: null },
  ] as const) {
    assert.deepEqual(locationFromHash(hashForLocation(location)), location);
  }
});

test("the dashboard and the browser are different links", () => {
  // They were the same place under ADR 0026, which is exactly what changed:
  // #/storage is now the dashboard and the browser is a screen below it.
  assert.equal(hashForLocation({ route: "storage", view: null, inspect: null }), "#/storage");
  assert.equal(
    hashForLocation({ route: "storage", view: "folders", inspect: null }),
    "#/storage/folders",
  );
  assert.equal(
    hashForLocation({ route: "storage", view: "developer", inspect: null }),
    "#/storage/developer",
  );
  // A view outside storage names nothing, so it stays out of the hash.
  assert.equal(hashForLocation({ route: "clean", view: "developer", inspect: null }), "#/clean");
});

test("a first run opens on onboarding, which nothing used to reach", () => {
  // The page was written and the hash resolved to it, and no code ever set that
  // hash: a fresh install went straight to a screen assuming a backend exists.
  assert.equal(firstLocation("", false), "onboarding");
  assert.equal(firstLocation("", true), DEFAULT_LOCATION);
});

test("a link is honoured even on a first run", () => {
  // Interrupting a deliberate arrival with a wizard loses where they were going.
  assert.deepEqual(firstLocation("#/clean", false), { route: "clean", view: null, inspect: null });
  assert.deepEqual(firstLocation("#/storage/developer", false), {
    route: "storage",
    view: "developer",
    inspect: null,
  });
});

test("onboarding can always be reopened by naming it", () => {
  assert.equal(firstLocation("#/onboarding", true), "onboarding");
});

test("an inspected application is a place, addressed by the node that names it", () => {
  assert.deepEqual(locationFromHash("#/storage/app/12"), {
    route: "storage",
    view: null,
    inspect: 12,
  });
  assert.equal(hashForLocation({ route: "storage", view: null, inspect: 12 }), "#/storage/app/12");
});

test("an id that names no application is the dashboard, not an empty Inspector", () => {
  for (const hash of [
    "#/storage/app",
    "#/storage/app/nonsense",
    "#/storage/app/-1",
    "#/storage/app/1.5",
  ]) {
    assert.deepEqual(locationFromHash(hash), DEFAULT_LOCATION, hash);
  }
});

test("inspecting outranks a view, because it replaces the screen", () => {
  // Both cannot show at once, and the hash carries whichever the window is on.
  assert.equal(
    hashForLocation({ route: "storage", view: "folders", inspect: 7 }),
    "#/storage/app/7",
  );
});
