# Nirmoka UI Readiness Audit

**Date:** 2026-08-18
**Scope:** Can the current architecture support the approved two-surface product design (Storage Overview + App Inspector) with cleanup, uninstall, and raw drill-down flows?

---

## Overall Verdict

**MOSTLY READY**

The architecture is sound. The hexagonal design (core → adapter → app → transport → frontend) with strict invariants means the UI can be rebuilt incrementally without touching the backend. The tree model, adapter trait, confirmation-token safety pattern, and virtualized rendering are all strong foundations.

The blocking gap is **application footprint attribution**. The current codebase cannot answer "Docker is 47.2 GB" — it can only answer "Docker.app is 1.8 GB" (from the scan tree) or "Mole says Docker is ~410 MB" (from Mole's rounded label). There is no code that associates `~/Library/Containers/com.docker.docker`, `~/Library/Caches/Docker`, or Docker's images/volumes back to the Docker application. This is the core product idea ("show the true storage footprint of an application"), and it does not exist yet.

Everything else — the scan pipeline, the tree model, cleanup/uninstall workflows, Finder integration, confirmation tokens, virtualized rendering — is either ready or needs incremental extension, not rewrite.

---

## 1. Current Architecture

### Application Framework

**Tauri v2** — Rust backend, React 19 webview frontend. The binary is small (single-digit MB) because it uses the OS webview, not Chromium.

### UI Framework

**React 19 + TypeScript + Tailwind v4 + shadcn/ui** (Radix primitives). No custom component library. Recharts for the donut chart. TanStack Virtual for list virtualization.

### State Management

**Pure state machines** via `useReducer` in `lib/engine/`. Four machines: scan, cleanup, trash, uninstall. Each is a typed reducer with events, fully testable without DOM. React Context (`AppContext`) holds transport, backends, scan state, and platform features.

### Routing/Navigation

**Hash-based**: `#/storage`, `#/clean`, `#/activity`, `#/help`. Storage has sub-views: `folders`, `developer`, `applications`. Navigation writes `window.location.hash`. Retired hashes redirect. No router library.

### Styling System

**Tailwind v4** with oklch CSS custom properties for light/dark themes. Design tokens in `index.css`. Platform-native font stack (`-apple-system, BlinkMacSystemFont, ...`). No web fonts loaded.

### Component System

**shadcn/ui** — CVA variants, Radix primitives (Dialog, Tooltip, Checkbox), `clsx` + `tailwind-merge` for class composition. Icons from `lucide-react`. No custom component library beyond shared patterns in `shared.tsx`.

### Native Bridge / Backend Architecture

**Tauri IPC** via `invoke()`. The `packages/transport` module is the only TypeScript code that imports `@tauri-apps/*`. Every IPC method is defined on a `Transport` interface with a mock implementation for dev without Rust. 34 transport methods.

### Filesystem Access Layer

All filesystem access goes through Rust. The frontend never touches the filesystem directly. Paths are reconstructed from tree node IDs in Rust (`tree.path_of(id)`). The `directories` crate provides platform-appropriate paths.

### Scanning Implementation

ncdu or gdu runs as a subprocess, producing ndjson. The wire format parser (`nirmoka-adapter::wire`) streams entries into a `Tree` via `WireSink`/`TreeSink`. The tree lives in Rust; the frontend receives windows of 100 rows at a time via `rows(scanId, parentId, sort, offset, limit)`.

### Cleanup Implementation

Mole's `mo clean --dry-run` produces a text file. The adapter parses it into `CleanupPreview` (categories with items: path + rounded size + item count). Execution runs `mo clean` (fresh discovery, not from preview). Confirmation tokens bind preview to execution.

### Uninstall Implementation

Mole's `mo uninstall --list` provides `InstalledApplication` (name, bundle_id, source, uninstall_name, path, reported_size). Preview via `mo uninstall --dry-run` produces `UninstallPreview` with per-app items showing scope (removed/system/reviewOnly). Execution runs `mo uninstall <name>` with `y\n` on stdin.

### Platform Abstraction

Runtime detection via `std::env::consts::OS`. `#[cfg(target_os)]` only in `crates/app` and `crates/adapter*` (never in `crates/core`). `PlatformFeatures` provides reveal label, quick look, trash label, window controls inset.

### Permission Handling

No Full Disk Access detection or prompting. Permissions are handled at the backend level (Mole's own authorization dialogs, `trash` crate's Automation permission for Finder). No sandbox entitlements.

### Process Execution

PATH augmented with package manager directories (`/opt/homebrew/bin`, `/usr/local/bin`, etc.). `CancelToken` (AtomicBool) shared between UI and worker thread. Watcher thread polls every 20ms and kills on cancellation. Stdin piped only for Mole's uninstall confirmation.

### Async/Background Jobs

Tauri's `async_runtime::spawn_blocking()` for long operations. Worker thread for scan progress events. Request identity counters prevent stale replies.

### Caching

Scan results are held in-memory in `AppState::tree`. No disk cache of scan results. Settings persisted to `settings.json` via atomic rename.

### Persistence

JSONL operation journal (`operations.jsonl`) for all destructive operations. Settings file for backend preference. No database.

---

## 2. Backend Capability for New UI

### Disk Level

| Capability           | Status      | Notes                                                                               |
| -------------------- | ----------- | ----------------------------------------------------------------------------------- |
| Total disk capacity  | **Exists**  | `volumeInfo(path)` → `VolumeInfo { totalBytes, usedBytes, freeBytes }` via `df -kP` |
| Used space           | **Exists**  | `VolumeInfo.usedBytes`                                                              |
| Free space           | **Exists**  | `VolumeInfo.freeBytes`                                                              |
| Mounted volume info  | **Exists**  | `VolumeInfo { name, mountPoint }` — resolves boot volume group naming               |
| Multiple volumes     | **Partial** | `df` output could list them, but `volumeInfo` only queries one path                 |
| APFS purgeable space | **Missing** | No `diskutil` or `tmutil` integration                                               |
| APFS snapshots       | **Missing** | Not relevant to current product scope                                               |

### Category Level

| Capability     | Status      | Notes                                                                                                                                 |
| -------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Apps total     | **Partial** | Scan tree finds `.app` bundles; Mole lists installed apps with sizes. No total category size from scan.                               |
| Personal Files | **Missing** | No heuristic to distinguish `~/Documents` contents from other files                                                                   |
| Development    | **Partial** | `DeveloperInventory` finds DerivedData, simulators, Archives, caches, git repos, node_modules — but only within the scanned directory |
| System         | **Partial** | `CleanupSystemScope` tells whether system cleanup is included, but no per-category breakdown of system vs user                        |
| Other          | **Missing** | Would be a catch-all; no current classification                                                                                       |
| Free Space     | **Exists**  | `VolumeInfo.freeBytes`                                                                                                                |

**Assessment:** Category breakdown will require a new heuristic layer. The scan tree provides raw directory sizes, but classifying directories into "Apps", "Personal Files", "Development", "System", "Other" requires path-based rules that do not exist yet. This is new code, not a bug fix.

### Application Footprint

| Capability          | Status      | Notes                                                                                       |
| ------------------- | ----------- | ------------------------------------------------------------------------------------------- |
| .app bundle size    | **Exists**  | Scan tree: `total_bytes` for any directory named `*.app`                                    |
| Bundle identifier   | **Partial** | Mole provides `bundle_id` in `InstalledApplication`. Scan tree does not parse Info.plist.   |
| Application Support | **Missing** | No attribution of `~/Library/Application Support/<bundle_id>` to an app                     |
| Caches              | **Missing** | No attribution of `~/Library/Caches/<bundle_id>` to an app                                  |
| Containers          | **Missing** | No attribution of `~/Library/Containers/<bundle_id>` to an app                              |
| Group Containers    | **Missing** | No attribution of `~/Library/Group Containers/<group_id>` to an app                         |
| Preferences         | **Missing** | No attribution of `~/Library/Preferences/<bundle_id>.plist` to an app                       |
| Saved State         | **Missing** | No attribution of `~/Library/Saved Application State/<bundle_id>.savedState` to an app      |
| External storage    | **Missing** | No attribution of app-specific external storage (Docker images, VMs, etc.)                  |
| Last used date      | **Missing** | ncdu wire format has no timestamps. Would need `mdls` or `LSUpdateDate` or Spotlight query. |
| Icon                | **Missing** | No `NSWorkspace.shared.icon(forFile:)` or `IconServices` integration                        |

**Assessment:** This is the primary gap. The product idea ("show the true storage footprint of an application") requires an **attribution engine** that:

1. Takes a list of installed applications (from Mole or scan tree)
2. For each app, discovers associated paths in `~/Library/` using bundle_id
3. Computes sizes for each path (from scan tree if available, or new filesystem walks)
4. Presents a per-app breakdown (Application, Application Support, Caches, Containers, etc.)

This engine does not exist. It needs to be built as a new Rust module, likely in `crates/app` (since it uses platform-specific path rules) or a new `crates/attribution` crate.

---

## 3. Application Attribution Model

### How Apps Are Currently Discovered

**From the scan tree** (`crates/app/src/inventory.rs`):

- Iterates all nodes, filters directories whose name ends with `.app`
- Extracts: node id, name (without `.app`), path (reconstructed), total_bytes, size_is_partial
- **No bundle_id**, no icon, no last-used, no Info.plist parsing
- Quality depends entirely on what was scanned (scanning `~` gives full paths; scanning `/Applications` gives bundle sizes only)

**From Mole** (`crates/adapter-mole/src/lib.rs`):

- Runs `mo uninstall --list` → JSON array of `InstalledApplication`
- Provides: name, bundle_id, source, uninstall_name, path, reported_size (rounded text)
- Size is deliberately NOT parsed into bytes (would invent precision)
- Only available when Mole is installed

**Frontend priority** (`applications-section.tsx`):

- If Mole inventory is available → it is primary (has bundle_id, uninstall names)
- Otherwise → scan-derived inventory is fallback (has byte counts, node IDs)

### What Metadata Is Available

| Field          | Scan Tree                | Mole                         |
| -------------- | ------------------------ | ---------------------------- |
| App name       | ✅ (from directory name) | ✅                           |
| Bundle ID      | ❌                       | ✅                           |
| Install source | ❌                       | ✅ ("App", "Homebrew", etc.) |
| Uninstall name | ❌                       | ✅                           |
| Absolute path  | ✅                       | ✅                           |
| Size (bytes)   | ✅ (rollup)              | ❌ (rounded text only)       |
| Last used      | ❌                       | ❌                           |
| Icon           | ❌                       | ❌                           |

### What Is Missing for App Footprint Attribution

The critical missing piece is the ability to answer:

> "Given bundle_id `com.docker.docker`, what directories in `~/Library/` belong to Docker, and how big are they?"

This requires:

1. **Path rules** for known library locations (`Application Support/<id>`, `Caches/<id>`, `Containers/<id>`, `Group Containers/<group>`, `Preferences/<id>.plist`, `Saved Application State/<id>.savedState`)
2. **Size computation** for each discovered path (ideally from the scan tree, falling back to `du` or tree walk)
3. **Bundle ID extraction** from Info.plist for scan-derived apps (when Mole is unavailable)
4. **Group container discovery** from `Info.plist` `WKAppGroups` or `com.apple.security.application-groups`

### Scalability

The path rules are well-known and stable (`~/Library/` structure has been consistent since macOS 10.x). A heuristic-based approach using bundle_id is scalable:

- Standard locations: `Application Support/<id>`, `Caches/<id>`, `Preferences/<id>.plist`, `Saved Application State/<id>.savedState`
- Container locations: `Containers/<id>`, `Group Containers/<group_id>`
- App-specific: varies by app (Docker's images, Xcode's DerivedData, etc.)

The "app-specific" category is where specialized providers/adapters come in (see section 4).

---

## 4. Developer-Storage Attribution

### Currently Supported

The `DeveloperInventory` in `crates/app/src/inventory.rs` finds 6 categories by pattern-matching directory names against the scan tree:

| Category          | Pattern                                               | Notes                                |
| ----------------- | ----------------------------------------------------- | ------------------------------------ |
| Xcode DerivedData | Name = `DerivedData`, path contains `Developer/Xcode` | Largest developer storage category   |
| Simulator Data    | Name = `CoreSimulator`, path contains `Developer`     | iOS simulator images                 |
| Xcode Archives    | Name = `Archives`, path contains `Developer/Xcode`    | Built archives                       |
| Developer Caches  | Name = `Caches` or `Logs`, path contains `Developer`  | Xcode-related caches                 |
| Node Modules      | Name = `node_modules`                                 | npm/pnpm/yarn dependencies           |
| Git Repository    | Name = `.git`                                         | Reports parent directory (repo root) |

### What Is Missing

| Category                | Status      | Notes                                                                                    |
| ----------------------- | ----------- | ---------------------------------------------------------------------------------------- |
| Android SDK / emulators | **Missing** | Would need path rules for `~/Library/Android/sdk`                                        |
| Gradle caches           | **Missing** | `~/.gradle/caches`                                                                       |
| Maven caches            | **Missing** | `~/.m2/repository`                                                                       |
| Homebrew                | **Missing** | `/opt/homebrew` or `/usr/local` size                                                     |
| Local LLM models        | **Missing** | `~/.cache/lm-studio`, `~/Library/Application Support/nomic.ai`                           |
| Docker                  | **Missing** | `~/Library/Containers/com.docker.docker` + `~/Library/Group Containers/group.com.docker` |
| Cargo / Rust            | **Missing** | `~/.cargo/registry`, `~/.rustup`                                                         |
| Go modules              | **Missing** | `~/go/pkg/mod`                                                                           |

### Architecture Allowance

The `DeveloperInventory` system is pattern-based and extensible. New categories can be added by:

1. Adding a variant to the `DeveloperCategory` enum
2. Adding a pattern-matching rule in `inventory.rs`
3. No trait changes, no adapter changes

This is incremental work, not architectural change. The question is whether these should be "developer categories" or whether Docker/Xcode/etc. should get their own specialized attribution (see the App Inspector design, which shows per-app breakdowns).

---

## 5. Cleanup Safety Model

### Current Metadata on CleanupItem

```rust
pub struct CleanupItem {
    pub path: PathBuf,
    pub reported_size: Option<String>,  // rounded text, not bytes
    pub item_count: u64,
}
```

That is all. Three fields.

### What the New UI Requires

The cleanup review design requires:

| Metadata                          | Current                       | Required                            |
| --------------------------------- | ----------------------------- | ----------------------------------- |
| Human-readable name               | ❌ (path only)                | ✅                                  |
| Size                              | ✅ (rounded text)             | ✅ (but needs bytes too for totals) |
| File count                        | ✅                            | ✅                                  |
| Path/location                     | ✅                            | ✅                                  |
| Why Nirmoka considers it safe     | ❌                            | ✅                                  |
| Whether it can be recreated       | ❌                            | ✅                                  |
| What app functionality it affects | ❌                            | ✅                                  |
| Risk level                        | ❌                            | ✅                                  |
| Selection state                   | Frontend-only                 | ✅                                  |
| Finder reveal                     | Frontend-only (needs node ID) | ✅                                  |

### Assessment

The current `CleanupItem` is a Mole-output parser — it reflects what Mole publishes, which is path + size + count. The new UI requires richer metadata that Mole does not provide.

Options:

1. **Extend `CleanupItem` in the adapter** — add optional fields (safety_reason, regeneratable, risk_level) that backends can populate when they have the information
2. **Nirmoka-side inference** — build a classification engine in `crates/app` that analyzes paths and known system locations to infer risk/safety/regenerability
3. **Hybrid** — extend the adapter type with optional fields, and add Nirmoka-side inference as a fallback

Option 3 is recommended. The adapter should be able to provide richer data when available (a future Mole version might), and Nirmoka should fill in what it can determine from path analysis.

---

## 6. Uninstall Capability

### Current State

| Capability                            | Status      | Notes                                                                            |
| ------------------------------------- | ----------- | -------------------------------------------------------------------------------- |
| Remove an .app                        | **Exists**  | Via Mole's `uninstall_execute`                                                   |
| Discover associated files             | **Exists**  | `UninstallPreview` provides per-app items with paths and scopes                  |
| Distinguish user data from disposable | **Partial** | `UninstallItemScope` has `Removed`/`System`/`ReviewOnly` — but no user-data flag |
| Retain selected data                  | **Missing** | Current flow removes everything Mole identifies; no per-item selection           |
| Remove everything                     | **Exists**  | Default behavior                                                                 |
| Request permissions when needed       | **Partial** | Mole handles its own authorization dialogs                                       |
| Fail safely                           | **Exists**  | Partial success is recorded; cancellation is an outcome                          |
| Report partial failures               | **Exists**  | `UninstallExecution.failed` lists apps that couldn't be removed                  |
| Avoid deleting unrelated files        | **Exists**  | Mole applies its own protections; Nirmoka never constructs paths                 |

### Gap: Per-Item Selection

The new UI design shows "Keep user data" vs "Remove everything" options. The current `UninstallPreview` reports per-item scope but does not support per-item selection — the user confirms the entire plan or cancels.

Adding per-item selection requires:

1. Extending the confirmation flow to accept a subset of items
2. Mole must support selective removal (currently it removes everything it identifies)
3. If Mole does not support selective removal, the UI must be honest about this limitation

---

## 7. Finder / macOS Integration

| Capability            | Status       | Notes                                                                             |
| --------------------- | ------------ | --------------------------------------------------------------------------------- |
| Reveal in Finder      | **Complete** | `open -R <path>` with path validation and canonicalization                        |
| Open application      | **Missing**  | `open -a <path>` would be straightforward to add                                  |
| File icons            | **Missing**  | Would need `NSWorkspace.shared.icon(forFile:)` via Tauri plugin or `fileicon` CLI |
| Application icons     | **Missing**  | Same as file icons                                                                |
| Quick Look            | **Complete** | `qlmanage -p` on worker thread                                                    |
| macOS permissions     | **Partial**  | Handled by backends and `trash` crate; no Nirmoka-level detection                 |
| Full Disk Access      | **Missing**  | No detection, no guidance UI                                                      |
| Trash                 | **Complete** | Finder route via `trash` crate; Put Back supported                                |
| Privileged operations | **Missing**  | Not needed for current scope                                                      |
| APFS purgeable space  | **Missing**  | Not in scope                                                                      |
| Snapshots             | **Missing**  | Not in scope                                                                      |

---

## 8. Scan Architecture

| Capability                     | Status       | Notes                                                                                    |
| ------------------------------ | ------------ | ---------------------------------------------------------------------------------------- |
| Progressive results            | **Complete** | Scan progress events via Tauri event system                                              |
| Scan progress                  | **Complete** | `ScanProgress { scanned, currentPath }`                                                  |
| Cancellation                   | **Complete** | `CancelToken` + watcher thread kills subprocess                                          |
| Rescanning                     | **Complete** | New scan replaces previous tree; history resets                                          |
| Caching previous results       | **Missing**  | No disk cache of scan results; in-memory only                                            |
| Partial refresh                | **Missing**  | Full rescan only; no incremental update                                                  |
| Large disks                    | **Partial**  | Tree is arena-backed (efficient); 500-node cap on inventories; MAX_ROWS=1000 for display |
| Hundreds of thousands of files | **Partial**  | Wire format parser streams; tree is Vec-backed; rollup is O(n)                           |
| Symlinks safely                | **Complete** | Canonicalized before validation; cycle detection in wire parser                          |
| Filesystem permission errors   | **Complete** | `read_error` flag on nodes; excluded entries tracked                                     |
| Excluded directories           | **Complete** | `ScanOptions::exclude` list                                                              |
| APFS peculiarities             | **Partial**  | Hardlinks handled (counted once); sparse files handled (apparent vs disk size)           |

---

## 9. Performance

### Current Bottlenecks for 512GB–2TB Mac

| Area                             | Risk       | Notes                                                                                                                           |
| -------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------- |
| **Wire format parsing**          | Low        | Streaming parser, one entry at a time                                                                                           |
| **Tree construction**            | Low        | `Vec<Node>` with `push()`, O(1) amortized                                                                                       |
| **Rollup computation**           | Low        | Single pass, O(n)                                                                                                               |
| **In-memory tree size**          | Medium     | A 2M-node scan: ~80MB (40 bytes/node × 2M). Fits in RAM but significant.                                                        |
| **Path reconstruction**          | Medium     | `tree.path_of(id)` walks parent pointers. Called per visible row (100 at a time), so acceptable.                                |
| **Inventories**                  | Low        | Capped at 500 rows; iterate full tree once                                                                                      |
| **Cleanup preview**              | Low        | Delegated to Mole subprocess                                                                                                    |
| **Uninstall preview**            | Low        | Delegated to Mole subprocess                                                                                                    |
| **Volume info**                  | Low        | Single `df` call                                                                                                                |
| **System status**                | Low        | Single Mole call                                                                                                                |
| **Frontend rendering**           | Low        | TanStack Virtual with 36px rows, 12-row overscan                                                                                |
| **New: App attribution**         | **High**   | If we need to walk `~/Library/` for each app, that's N filesystem operations per scan. Needs caching or scan-tree-based lookup. |
| **New: Category classification** | **Medium** | Every node needs path-based classification. O(n) per scan, but adds complexity to the hot path.                                 |

### Recommendations

1. The tree stays in Rust (invariant 5). Do not ship it to the frontend.
2. App attribution should be computed once per scan and stored in `AppState`, not recomputed on every render.
3. Category classification should happen during tree construction (single pass), not as a post-processing step.
4. The 500-row cap on inventories may need to be raised or made paginated for the new UI (more apps visible).

---

## 10. Code Quality

### Separation of Concerns

**Strong.** The five invariants are enforced by the workspace manifest and CI. `core` has no GUI dependency. `app` is a translation layer. `transport` is the only Tauri-aware JS module.

### UI/Backend Boundaries

**Strong.** The Transport interface with mock implementation is a clean boundary. DTOs are separate from domain types (ADR 0010). ts-rs generation is committed and CI-checked.

### Component Structure

**Good.** Shared patterns in `shared.tsx`. Feature components are self-contained. State machines are pure reducers. Keyboard navigation is a pure function.

### Domain Models

**Good but minimal.** The `Node` type is lean (name, kind, bytes, flags). The adapter types are well-documented with explicit design rationale (e.g., why `reported_size` is text not bytes).

### Service Abstractions

**Good.** The adapter trait is the right abstraction. The registry with ability-based resolution is well-designed. The confirmation token pattern is sound.

### Naming

**Consistent.** Rust code follows standard conventions. TypeScript uses camelCase. Types are descriptive (`CleanupPreview`, `UninstallItemScope`).

### Error Handling

**Strong.** `AdapterError` covers all failure modes. The "infallible execution" pattern (cancellation and backend failure return `Ok` with completion status) is correct for irreversible operations. Journal write failures are reported beside results, not as operation failures.

### Testability

**Good.** State machines are pure functions (testable via `node --test`). Contract tests verify adapter behavior. Wire format tests verify parser correctness. The mock transport enables frontend development without Rust.

---

## Capability Matrix

| Capability               | Existing | Partial | Missing | Notes                                                                         |
| ------------------------ | -------- | ------- | ------- | ----------------------------------------------------------------------------- |
| Disk usage               | ✅       |         |         | `volumeInfo` via `df -kP`                                                     |
| Category breakdown       |          | ✅      |         | `DeveloperInventory` for dev; no Apps/Personal/System/Other classification    |
| App discovery            | ✅       |         |         | Scan tree + Mole inventory                                                    |
| True app footprint       |          |         | ❌      | **Primary gap.** No attribution of Library paths to apps                      |
| Related file attribution |          | ✅      |         | Mole's uninstall preview shows per-app items; no pre-computed attribution     |
| Cleanup classification   |          |         | ❌      | `CleanupItem` has path+size+count only; no risk/safety/regeneratable metadata |
| Cleanup execution        | ✅       |         |         | Mole cleanup with confirmation tokens                                         |
| App uninstall            | ✅       |         |         | Mole uninstall with confirmation tokens                                       |
| Preserve user data       |          | ✅      |         | `UninstallItemScope` exists; no per-item selection UI                         |
| Raw file exploration     | ✅       |         |         | Tree view with virtualized paged loading                                      |
| Reveal in Finder         | ✅       |         |         | `open -R` with path validation                                                |
| Quick Look               | ✅       |         |         | `qlmanage -p` on worker thread                                                |
| Scan progress            | ✅       |         |         | Event-based progress reporting                                                |
| Permissions              |          | ✅      |         | Backend-level; no Full Disk Access detection                                  |
| Trash / Put Back         | ✅       |         |         | Finder route via `trash` crate                                                |
| Confirmation tokens      | ✅       |         |         | One-time tokens with version binding                                          |
| Operation journal        | ✅       |         |         | JSONL append-only journal                                                     |
| Icons / last-used        |          |         | ❌      | No Info.plist parsing, no `mdls`, no NSWorkspace icons                        |

---

## Blocking Gaps

### Gap 1: Application Footprint Attribution

**Problem:** The product's core idea is "show the true storage footprint of an application." The current codebase cannot associate Library paths (Application Support, Caches, Containers, Preferences, Saved State) with specific applications.

**Why the new UI requires it:** The Storage Overview shows "Docker — 47.2 GB." The Inspector shows "Application: 1.8 GB, Containers: 31.4 GB, Images: 8.2 GB, Volumes: 4.6 GB, Logs & Cache: 1.2 GB." Neither is possible without attribution.

**Current implementation:** No attribution code exists. `ApplicationInventory` reports only `.app` bundle sizes. Mole provides `bundle_id` but no Library path association.

**Minimal change needed:**

1. New Rust module (`crates/app/src/attribution.rs` or `crates/attribution/`) that:
   - Takes a bundle_id
   - Discovers associated paths in `~/Library/` using well-known location rules
   - Reports sizes (from scan tree if the path was scanned, or from `du`/stat if not)
2. New Tauri command: `app_footprint(scanId, nodeId) → AppFootprint`
3. New DTO: `AppFootprint { components: Vec<StorageComponent> }` where `StorageComponent { name, path, bytes, category }`
4. Frontend: Inspector page consumes `AppFootprint` to show the breakdown

### Gap 2: Cleanup Item Metadata

**Problem:** `CleanupItem` has only path, rounded size, and item count. The new UI needs risk level, safety reason, regeneratable flag, and user-generated flag.

**Why the new UI requires it:** The cleanup review shows "why Nirmoka considers it safe" and "whether it can be recreated." Without this, the review is just a list of paths.

**Current implementation:** Three fields per item.

**Minimal change needed:**

1. Extend `CleanupItem` with optional fields: `risk_level: Option<RiskLevel>`, `safety_reason: Option<String>`, `regeneratable: Option<bool>`, `user_generated: Option<bool>`
2. Nirmoka-side inference engine that classifies paths by known system locations
3. Mole adapter populates what it can; Nirmoka fills the rest

### Gap 3: Category Classification

**Problem:** The Storage Overview needs to show broad categories (Apps, Personal Files, Development, System, Other, Free Space). The scan tree has no category classification.

**Why the new UI requires it:** The overview shows a visual storage distribution by category.

**Current implementation:** `DeveloperInventory` classifies 6 developer-specific patterns. No other classification exists.

**Minimal change needed:**

1. Path-based classification rules applied during tree traversal or as a post-scan computation
2. Rules: `.app` bundles → Apps; `~/Documents`, `~/Desktop`, `~/Downloads` → Personal Files; DeveloperInventory matches → Development; `/System`, `/Library` → System; everything else → Other
3. New command: `categoryBreakdown(scanId) → CategoryBreakdown`

---

## Existing Functionality We Should Preserve

1. **Tree model and windowing** (ADR 0005, 0011) — Arena-backed tree, server-side sorting/paging, frontend holds only a window. Do not ship the whole tree.

2. **Confirmation token pattern** — Two-call boundary (prepare → confirm) with one-time tokens. This is the safety model. Do not weaken it.

3. **Wire format parser** (ADR 0008) — Lives in `crates/adapter`, shared by ncdu and gdu. Streaming, tested against fixtures.

4. **Adapter trait and registry** (ADR 0013) — Ability-based resolution with platform defaults. Do not hardcode backend selection.

5. **"Degrade, don't lie" principle** — `Unsupported` is product truth. Do not fake capabilities.

6. **Preview ≠ execution** (ADR 0020, 0027) — Backends re-discover on execution. The UI must not promise preview matches execution.

7. **Infallible execution** — Once a subprocess is running, every outcome (including cancellation) is recorded, not errored.

8. **Request identity** — Monotonically increasing counters prevent stale replies.

9. **Mock transport** — Enables frontend development without Rust.

10. **GPL discipline** — Never copy Mole's data tables. The process boundary is the protection.

---

## Proposed Domain Model

These are incremental additions to the existing model, not replacements.

### New Types (Rust side)

```rust
// crates/app/src/attribution.rs (new module)

/// A storage component associated with an application.
pub struct StorageComponent {
    pub name: String,           // "Application", "Containers", "Caches", etc.
    pub path: String,           // Absolute path
    pub bytes: u64,             // Disk usage
    pub file_count: u64,        // Number of files
    pub source: ComponentSource, // How this was discovered
}

pub enum ComponentSource {
    ScanTree,       // Found in the scanned tree (exact size)
    FilesystemWalk, // Computed via du/stat (may be approximate)
    Backend,        // Reported by Mole or another backend
}

/// The full storage footprint of an application.
pub struct AppFootprint {
    pub name: String,
    pub bundle_id: Option<String>,
    pub app_path: String,
    pub app_bytes: u64,
    pub components: Vec<StorageComponent>,
    pub total_bytes: u64,
    pub reclaimable_bytes: Option<u64>,
}

// crates/app/src/categories.rs (new module)

pub struct CategoryBreakdown {
    pub categories: Vec<StorageCategory>,
    pub total_scanned: u64,
    pub free_space: u64,
}

pub struct StorageCategory {
    pub name: String,           // "Apps", "Personal Files", "Development", "System", "Other"
    pub bytes: u64,
    pub icon: String,           // Lucide icon name
    pub top_consumers: Vec<CategoryConsumer>,  // Top 5 entries in this category
}

pub struct CategoryConsumer {
    pub name: String,
    pub bytes: u64,
    pub path: String,
    pub node_id: Option<u32>,  // For tree navigation
}
```

### Extended Types (modifications to existing)

```rust
// Extend CleanupItem (crates/adapter/src/cleanup.rs)
pub struct CleanupItem {
    pub path: PathBuf,
    pub reported_size: Option<String>,
    pub item_count: u64,
    // New optional fields:
    pub risk_level: Option<CleanupRisk>,
    pub safety_reason: Option<String>,
    pub regeneratable: Option<bool>,
    pub user_generated: Option<bool>,
}

pub enum CleanupRisk {
    Low,      // Cache, temp file, regeneratable
    Medium,   // Logs, old data, likely safe
    High,     // User data, application state, review needed
}
```

---

## Implementation Plan

### Step 1: App Footprint Attribution Engine

**Goal:** Given a scanned tree and a list of installed apps, compute per-app storage footprint.

**Files affected:**

- `crates/app/src/attribution.rs` (new) — path discovery rules, size computation
- `crates/app/src/commands.rs` — new `app_footprint` command
- `crates/app/src/dto.rs` — new `AppFootprintDto`, `StorageComponentDto`
- `packages/transport/src/index.ts` — new `appFootprint()` method
- `packages/transport/src/generated/bindings.ts` — regenerated types

**Backend changes:** New Rust module. No adapter trait changes.

**Risk:** Medium. The path rules are well-known but need careful implementation for edge cases (apps with multiple bundle IDs, apps with non-standard Library locations).

**Test strategy:**

- Unit tests for path discovery rules (mock bundle_id → expected paths)
- Integration test with a real scan tree (verify sizes match)
- Fixture-based test with recorded Library structures

### Step 2: Category Classification

**Goal:** Classify every scanned node into broad categories.

**Files affected:**

- `crates/app/src/categories.rs` (new) — classification rules
- `crates/app/src/commands.rs` — new `category_breakdown` command
- `crates/app/src/dto.rs` — new types
- `packages/transport/src/index.ts` — new method
- `packages/transport/src/generated/bindings.ts` — regenerated

**Backend changes:** New Rust module.

**Risk:** Low. Classification is path-based and deterministic.

**Test strategy:**

- Unit tests for classification rules
- Contract test: every node in a fixture tree gets a category

### Step 3: Extend CleanupItem Metadata

**Goal:** Add risk level, safety reason, regeneratable flag to cleanup items.

**Files affected:**

- `crates/adapter/src/cleanup.rs` — extend `CleanupItem`
- `crates/adapter-mole/src/lib.rs` — populate new fields where possible
- `crates/app/src/cleanup.rs` — Nirmoka-side inference for missing fields
- `crates/app/src/dto.rs` — extend DTOs
- `packages/transport/src/generated/bindings.ts` — regenerated

**Backend changes:** Adapter type extension with optional fields (backward-compatible).

**Risk:** Low. Optional fields are additive.

**Test strategy:**

- Existing contract tests continue to pass (new fields are optional)
- New unit tests for inference rules

### Step 4: Storage Overview Page

**Goal:** Replace the current scan-centric Storage page with the new two-surface design.

**Files affected:**

- `apps/desktop/src/pages/storage-page.tsx` — rewrite
- `apps/desktop/src/pages/sections/summary-section.tsx` — rewrite as StorageOverview
- New components: `StorageUsageBar`, `StorageCategorySummary`, `StorageConsumerRow`
- `apps/desktop/src/pages/sections/applications-section.tsx` — will become part of Inspector flow

**Frontend changes:** Major rewrite of Storage page. Backend provides all data via new commands.

**Risk:** High. This is the largest UI change. Should be done incrementally (build new components alongside existing ones, switch when ready).

**Test strategy:**

- Visual testing with mock transport
- State machine tests for new selection/navigation states

### Step 5: App Inspector Page

**Goal:** When user clicks an app in the overview, show the Inspector with footprint breakdown.

**Files affected:**

- New: `apps/desktop/src/pages/inspector-page.tsx`
- New: `apps/desktop/src/pages/sections/footprint-section.tsx`
- New components: `AppHeader`, `AppFootprintSummary`, `StorageComponentRow`
- `apps/desktop/src/pages/sections/raw-drill-down.tsx` — breadcrumb file browser

**Frontend changes:** New page with sub-sections.

**Risk:** Medium. Depends on Step 1 (attribution engine) and Step 4 (overview navigation).

**Test strategy:**

- Component tests with mock footprint data
- Navigation flow tests (overview → inspector → drill-down)

### Step 6: Raw File Drill-down

**Goal:** From the Inspector, allow progressive inspection of individual files/folders.

**Files affected:**

- `apps/desktop/src/components/tree-view.tsx` — extend or create inspector-specific variant
- New: `apps/desktop/src/components/breadcrumb.tsx`
- New: `apps/desktop/src/components/file-details-panel.tsx`

**Frontend changes:** Reuse existing tree view infrastructure (virtualization, keyboard nav) with Inspector-specific chrome.

**Risk:** Low. The tree view already supports this pattern.

**Test strategy:**

- Reuse existing tree view tests
- New tests for breadcrumb navigation

### Step 7: Cleanup Review with Rich Metadata

**Goal:** Show the new cleanup review UI with safety classification, regeneratable flags, and "What will NOT be removed."

**Files affected:**

- `apps/desktop/src/pages/clean-page.tsx` — extend with new metadata display
- New components: `CleanupCandidateRow`, `SafetyIndicator`, `ReclaimSummary`
- `apps/desktop/src/components/shared.tsx` — extend safety banner patterns

**Frontend changes:** Extend existing cleanup page with richer rendering.

**Risk:** Low. Depends on Step 3 (metadata extension).

**Test strategy:**

- Component tests with mock cleanup data including risk levels
- Confirmation flow tests

### Step 8: Uninstall Sheet from Inspector

**Goal:** Uninstall as a modal/sheet from the Inspector, not a separate page.

**Files affected:**

- `apps/desktop/src/pages/inspector-page.tsx` — add uninstall trigger
- `apps/desktop/src/components/uninstall-review.tsx` — extend with "Keep user data" / "Remove everything" options
- New: `apps/desktop/src/components/uninstall-option.tsx`

**Frontend changes:** Extend existing uninstall flow.

**Risk:** Medium. Per-item selection is a new UX pattern. Mole may not support selective removal.

**Test strategy:**

- Confirmation flow tests with selection variants
- Mole compatibility tests (what happens when selective removal is requested)

---

## What NOT to Do

1. **Do not rewrite the tree model.** It is arena-backed, efficient, and well-tested.
2. **Do not rewrite the adapter trait.** Extend it with new capabilities if needed.
3. **Do not rewrite the transport layer.** Add new methods to the existing interface.
4. **Do not rewrite the state machines.** Extend them with new states/events.
5. **Do not rewrite the confirmation token pattern.** It is the safety model.
6. **Do not add a router library.** Hash-based routing is sufficient for 2-3 pages.
7. **Do not add a component library.** shadcn/ui is sufficient.
8. **Do not add AI/ML for classification.** Path-based heuristics are deterministic and explainable.
9. **Do not hardcode app-specific rules in the UI.** All classification lives in Rust.
10. **Do not ship the tree to the frontend.** Invariant 5 holds.

---

## Summary

The architecture is ready for the new UI. The main work is:

1. **Build the attribution engine** (new Rust module, ~500-800 lines)
2. **Build category classification** (new Rust module, ~200-300 lines)
3. **Extend cleanup metadata** (adapter type extension + inference, ~200 lines)
4. **Build the Storage Overview** (frontend rewrite, ~800-1200 lines)
5. **Build the Inspector** (new page, ~600-1000 lines)
6. **Build raw drill-down** (extend tree view, ~300-500 lines)
7. **Extend cleanup review** (frontend extension, ~400-600 lines)
8. **Extend uninstall flow** (frontend extension, ~300-500 lines)

Total estimated new/changed code: ~3,300-5,100 lines across Rust and TypeScript. This is incremental work on a solid foundation, not a rewrite.
