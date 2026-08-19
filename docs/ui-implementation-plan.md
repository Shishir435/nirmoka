# Implementation Plan: Two-Surface UI

Incremental plan to reach the approved product design. Each step is independently shippable and testable.

---

## Phase 1: Attribution Engine (Foundation)

Everything else depends on knowing what storage belongs to which app.

### Step 1.1: Bundle ID Discovery

**Goal:** Extract bundle_id from Info.plist for scan-derived `.app` bundles.

**Files:**

- `crates/app/src/attribution.rs` (new) — `fn bundle_id_from_plist(app_path: &Path) -> Option<String>`
- Reads `Contents/Info.plist`, parses `CFBundleIdentifier`
- Uses `plist` crate (add to `crates/app/Cargo.toml`)
- Falls back gracefully if plist is missing or unparseable

**Why first:** Every subsequent step needs bundle_id. Mole provides it; scan-derived apps do not. This closes that gap.

**Test:** Unit test with a mock plist. Integration test with a real `.app` bundle.

### Step 1.2: Library Path Discovery

**Goal:** Given a bundle_id, discover associated paths in `~/Library/`.

**Files:**

- `crates/app/src/attribution.rs` — extend with `fn associated_paths(bundle_id: &str) -> Vec<KnownPath>`
- Known locations:
  - `~/Library/Application Support/{bundle_id}`
  - `~/Library/Caches/{bundle_id}`
  - `~/Library/Containers/{bundle_id}`
  - `~/Library/Preferences/{bundle_id}.plist`
  - `~/Library/Saved Application State/{bundle_id}.savedState`
  - `~/Library/HTTPStorages/{bundle_id}`
  - `~/Library/WebKit/{bundle_id}`
  - `~/Library/Logs/{bundle_id}`
- Each path is checked for existence before reporting
- Returns `Vec<KnownPath>` with name, path, and category

**Test:** Unit test with mock home directory. Verify existence checks.

### Step 1.3: Size Computation from Scan Tree

**Goal:** For each discovered path, find its size in the scan tree (if it was scanned).

**Files:**

- `crates/app/src/attribution.rs` — `fn size_from_tree(tree: &Tree, path: &Path) -> Option<u64>`
- Walks the tree to find the node matching the path
- Returns `total_bytes` if found, `None` if not in the scanned set

**Why from tree:** Avoids additional filesystem walks. If the user scanned `~`, all Library paths are in the tree. If they scanned something else, sizes for unscanned paths are unavailable (reported as such).

**Test:** Unit test with a small tree containing known paths.

### Step 1.4: Filesystem Walk Fallback

**Goal:** For paths not in the scan tree, compute size via `du` or tree walk.

**Files:**

- `crates/app/src/attribution.rs` — `fn size_from_filesystem(path: &Path) -> Option<u64>`
- Uses `std::fs::read_dir` recursive walk (not `du` subprocess — avoids process overhead)
- Counts files and sums `blocks * 512` (disk usage, not apparent size)
- Respects symlinks (does not follow)
- Reports errors gracefully (permission denied → `None`)

**Test:** Unit test with temp directory. Verify size matches `du -s`.

### Step 1.5: AppFootprint Command

**Goal:** New Tauri command that returns the full footprint of one app.

**Files:**

- `crates/app/src/commands.rs` — new `app_footprint(scan_id, node_id) -> AppFootprintDto`
- `crates/app/src/dto.rs` — new `AppFootprintDto`, `StorageComponentDto`
- `crates/app/src/attribution.rs` — orchestration function
- `packages/transport/src/index.ts` — new `appFootprint(scanId, nodeId)` method

**Flow:**

1. Resolve node from scan tree → get path and name
2. Extract bundle_id (from plist or Mole inventory)
3. Discover associated paths
4. Compute sizes (from tree, then filesystem fallback)
5. Return `AppFootprint` with components

**Test:** Integration test with a scan tree containing a known app and its Library paths.

### Step 1.6: Extend Transport and Frontend Types

**Goal:** Generate TypeScript types for the new commands.

**Files:**

- `pnpm types` to regenerate `bindings.ts`
- Verify new types appear

**Test:** `pnpm typecheck` passes.

---

## Phase 2: Category Classification

### Step 2.1: Classification Rules

**Goal:** Classify every scanned node into broad categories.

**Files:**

- `crates/app/src/categories.rs` (new)
- Rules (applied by path):
  - **Apps:** Directories ending in `.app`, or under `/Applications`
  - **Personal Files:** `~/Documents`, `~/Desktop`, `~/Downloads`, `~/Movies`, `~/Music`, `~/Pictures`
  - **Development:** Matches `DeveloperInventory` patterns + `~/.cargo`, `~/.rustup`, `~/go`, `~/.gradle`, `~/.m2`
  - **System:** Under `/System`, `/Library`, `/usr`, `/bin`, `/sbin`
  - **Other:** Everything else

**Test:** Unit test with path strings. Verify deterministic classification.

### Step 2.2: Category Breakdown Command

**Goal:** New Tauri command returning category sizes and top consumers.

**Files:**

- `crates/app/src/commands.rs` — new `category_breakdown(scan_id) -> CategoryBreakdownDto`
- `crates/app/src/dto.rs` — new types
- `packages/transport/src/index.ts` — new method

**Flow:**

1. Iterate all nodes in tree
2. Classify each node
3. Sum bytes per category
4. Find top 5 consumers per category
5. Include volume free space

**Test:** Integration test with fixture tree. Verify sums match expected.

---

## Phase 3: Storage Overview

### Step 3.1: Storage Overview Components

**Goal:** Build the reusable components for the new Storage Overview.

**Files:**

- New: `apps/desktop/src/components/storage-usage-bar.tsx` — horizontal stacked bar
- New: `apps/desktop/src/components/storage-category-summary.tsx` — category row with bar + size
- New: `apps/desktop/src/components/storage-consumer-row.tsx` — app/entry row with name + size + bar

**Design:**

- `StorageUsageBar`: horizontal bar showing used/free by category, hover to see breakdown
- `StorageCategorySummary`: category name, bar, size, expandable to show top consumers
- `StorageConsumerRow`: app name, size, bar, click to open Inspector

**Test:** Component tests with mock data.

### Step 3.2: Storage Overview Page

**Goal:** Replace the current Storage page with the new two-surface design.

**Files:**

- `apps/desktop/src/pages/storage-page.tsx` — rewrite
- `apps/desktop/src/pages/sections/summary-section.tsx` — rewrite
- Remove old tab bar (Folders / Developer / Applications) — these become Inspector sub-views
- Add navigation state: `view: "overview" | "inspector" | { inspector: { nodeId, name } }`

**Design:**

- Top: volume name + usage bar
- Middle: category breakdown (Apps, Personal Files, Development, System, Other)
- Bottom: biggest storage users list (sorted by size, click to inspect)
- Scan/rescan action stays
- Search within the overview

**Risk:** This is the largest single change. Build incrementally:

1. Build new components first (3.1)
2. Build new page alongside old one
3. Switch when ready

**Test:** Visual testing with mock transport. End-to-end flow test.

---

## Phase 4: App Inspector

### Step 4.1: Inspector Page

**Goal:** When user clicks an app, show its full footprint.

**Files:**

- New: `apps/desktop/src/pages/inspector-page.tsx`
- New: `apps/desktop/src/pages/sections/footprint-section.tsx`
- New: `apps/desktop/src/components/app-header.tsx` — app name, icon, total size
- New: `apps/desktop/src/components/app-footprint-summary.tsx` — 4-card grid of top components
- New: `apps/desktop/src/components/storage-component-row.tsx` — component row with path + size

**Design:**

- Left sidebar (contextual, not global): app name, total footprint, actions (Reveal, Open, Uninstall)
- Main area: footprint breakdown (Application, Containers, Caches, etc.)
- Each component is expandable to show individual paths
- "Potentially reclaimable" highlighted
- "Last used" if available

**Test:** Component tests. Navigation flow test (overview → inspector → back).

### Step 4.2: Open Application Command

**Goal:** Add "Open App" action.

**Files:**

- `crates/app/src/commands.rs` — new `open_application(path)` command
- Uses `open -a <path>` on macOS
- `packages/transport/src/index.ts` — new `openApplication(path)` method

**Test:** Unit test. Manual test on macOS.

---

## Phase 5: Raw File Drill-down

### Step 5.1: Inspector Tree View

**Goal:** From the Inspector, drill into individual files/folders.

**Files:**

- `apps/desktop/src/components/tree-view.tsx` — extract core logic into reusable hook
- New: `apps/desktop/src/components/inspector-tree.tsx` — tree view variant for Inspector context
- New: `apps/desktop/src/components/breadcrumb.tsx` — breadcrumb navigation
- New: `apps/desktop/src/components/file-details-panel.tsx` — right panel showing selected item details

**Design:**

- Breadcrumb: Docker → Containers → com.docker.docker → Docker.raw
- Tree: virtualized list of children (reuse existing `useDirectory` hook)
- Details panel: size, modified date, item count, safety classification
- Actions: Reveal in Finder, Quick Look

**Test:** Reuse existing tree view tests. New breadcrumb tests.

---

## Phase 6: Cleanup Review Enhancement

### Step 6.1: Extended CleanupItem Display

**Goal:** Show risk level, safety reason, and regeneratable flag in cleanup review.

**Files:**

- `apps/desktop/src/pages/clean-page.tsx` — extend with new metadata
- New: `apps/desktop/src/components/cleanup-candidate-row.tsx` — row with safety indicator
- New: `apps/desktop/src/components/safety-indicator.tsx` — shield icon + risk level
- New: `apps/desktop/src/components/reclaim-summary.tsx` — total reclaimable with breakdown

**Design:**

- Each cleanup candidate shows: name, size, path, safety indicator (low/medium/high risk)
- Expandable to show: why it's safe, whether it's regeneratable, what app it affects
- "What will NOT be removed" section at the bottom
- Destructive CTA shows exact amount: "Remove 10.9 GB"

**Test:** Component tests with mock data including risk levels.

---

## Phase 7: Uninstall Sheet

### Step 7.1: Uninstall from Inspector

**Goal:** Uninstall as a modal/sheet from the Inspector.

**Files:**

- `apps/desktop/src/pages/inspector-page.tsx` — add uninstall button
- `apps/desktop/src/components/uninstall-review.tsx` — extend with options
- New: `apps/desktop/src/components/uninstall-option.tsx` — "Keep user data" / "Remove everything"

**Design:**

- Sheet slides in from right (or modal dialog)
- Shows app name, total size, per-component breakdown
- Two radio options: "Keep user data" (remove app + disposable support) / "Remove everything"
- "Remove everything" only recommended when confidence is high
- If uncertainty about data classification, surface it: "Review recommended"
- Final CTA: "Uninstall {app_name}" (exact name, not "Clean Now")

**Test:** Confirmation flow tests. Mole compatibility tests.

---

## Dependency Graph

```
Phase 1 (Attribution)
  └── Phase 2 (Categories)
       └── Phase 3 (Overview)
            └── Phase 4 (Inspector)
                 ├── Phase 5 (Drill-down)
                 ├── Phase 6 (Cleanup)
                 └── Phase 7 (Uninstall)
```

Phases 5, 6, 7 are independent of each other but all depend on Phase 4.

---

## Estimated Size

| Phase          | Rust             | TypeScript       | Total            |
| -------------- | ---------------- | ---------------- | ---------------- |
| 1: Attribution | ~600 lines       | ~100 lines       | ~700 lines       |
| 2: Categories  | ~250 lines       | ~50 lines        | ~300 lines       |
| 3: Overview    | ~50 lines        | ~800 lines       | ~850 lines       |
| 4: Inspector   | ~50 lines        | ~700 lines       | ~750 lines       |
| 5: Drill-down  | ~0 lines         | ~400 lines       | ~400 lines       |
| 6: Cleanup     | ~100 lines       | ~400 lines       | ~500 lines       |
| 7: Uninstall   | ~50 lines        | ~350 lines       | ~400 lines       |
| **Total**      | **~1,100 lines** | **~2,800 lines** | **~3,900 lines** |

This is incremental work on a solid foundation. No rewrites required.
