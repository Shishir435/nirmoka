# Context: GUI Design for Mole CLI

You are helping design the user experience for **Nirmoka**, a cross-platform desktop GUI for disk analysis and cleanup. It wraps existing CLI tools (ncdu, gdu, Mole, rip) as subprocesses behind an adapter layer. The current focus is the **Mole adapter** — a macOS-only cleanup and app-uninstall CLI.

This document gives you enough context to provide specific, actionable UX feedback. Read it thoroughly before responding.

---

## 1. What Mole Is

Mole (`mo`) is a macOS-only disk cleanup utility. It is **not** a disk scanner — it is a removal engine with its own curated safety rules. Nirmoka drives it as a subprocess. Mole's binary is invoked as `mo`, and Nirmoka gates it to version `>=1.48, <2.0`.

### Mole commands Nirmoka uses

| Command                         | Purpose                                                        | Output format                                                       | Timing                  |
| ------------------------------- | -------------------------------------------------------------- | ------------------------------------------------------------------- | ----------------------- |
| `mo status --json`              | One-shot system health snapshot                                | JSON                                                                | Fast (~1s)              |
| `mo uninstall --list`           | List installed apps with uninstall identifiers                 | JSON (when stdout is not a terminal)                                | Fast (~2s)              |
| `mo uninstall --dry-run <name>` | Preview what removing an app would delete                      | Human-readable text (not JSON)                                      | Medium (~3-5s)          |
| `mo uninstall <name>`           | Remove an application (Mole prompts for confirmation on stdin) | Human-readable text                                                 | Medium (~3-10s per app) |
| `mo clean --dry-run`            | Preview cleanup opportunities (caches, logs, old files)        | Human-readable text file written to `~/.config/mole/clean-list.txt` | Slow (~10-30s)          |
| `mo clean`                      | Execute cleanup removals (fresh discovery, not from preview)   | Human-readable text                                                 | Slow (~10-30s)          |

### Critical Mole behaviors

1. **Mole re-discovers on execution.** When you run `mo clean` or `mo uninstall`, Mole re-discovers eligible items from scratch. The preview is a snapshot in time; the execution sees a potentially different state. Nirmoka never forwards preview paths to execution.

2. **Mole applies its own safety rules.** It has curated lists of protected paths and cleanup targets. Nirmoka never reimplements these rules. If Mole says something is safe to remove, that is Mole's judgment, not Nirmoka's.

3. **Mole prompts for confirmation via stdin.** The `uninstall` command expects `y\n` on stdin to proceed. Nirmoka relays this — the user confirms in the GUI, and Nirmoka writes `y\n` to Mole's stdin.

4. **Mole routes to Trash by default.** Uninstall operations move files to Trash, not permanent deletion. The `--permanent` flag exists but Nirmoka never passes it.

5. **Output is human-readable, not structured.** Cleanup and uninstall output are text, not JSON. Nirmoka parses this text into structured types. The raw transcript is always kept alongside the parsed form so the user can see exactly what Mole said.

---

## 2. Current App Architecture

### Stack

| Layer            | Technology                                                                       |
| ---------------- | -------------------------------------------------------------------------------- |
| Shell            | Tauri v2 — Rust backend, webview frontend                                        |
| Core             | Rust — domain model, tree, sizes, policy (no GUI framework)                      |
| Adapters         | Rust — trait-based, one per backend (ncdu, gdu, Mole, rip)                       |
| Frontend         | React 19 + TypeScript + Tailwind v4 + shadcn/ui                                  |
| Wire format      | ncdu JSON export (for scanning), Mole's text/JSON (for cleanup/uninstall/status) |
| Package managers | Cargo (Rust), pnpm (JS)                                                          |

### Layout (ADR 0026)

The window has three sidebar destinations:

- **Storage** — The disk scanner (ncdu). Shows a virtualized tree browser with sub-views: Folders, Developer, Applications. System status is a collapsible section here.
- **Clean** — The Mole cleanup workflow. Standalone page for previewing and running cleanup.
- **Activity** — A merged journal timeline of all past operations (trash, cleanup, uninstall, delete).

One scan bar lives in the shell header (Rust holds exactly one scan at a time). Help is a header button. Settings is a dialog. Onboarding is a separate first-run route.

### Routing

Hash-based: `#/storage`, `#/clean`, `#/activity`. Retired hashes (`#/overview`, `#/space`, `#/status`, `#/developer`, `#/applications`) redirect to the appropriate new location.

### State Management

- **Pure state machines** in `lib/engine/` — `cleanup-flow.ts`, `uninstall-flow.ts`, `trash-flow.ts`, `scan-machine.ts`. These are `useReducer` reducers with typed events. Fully testable without DOM.
- **React Context** (`AppContext`) holds transport, backends, scan state, platform features.
- **Request identity counters** — Every async flow uses a `useRef(0)` counter incremented per request. Late replies from abandoned requests are silently dropped.

### Design System

- shadcn/ui components: Button, Card, Dialog, Badge, Tooltip, Skeleton, Checkbox, Input
- Tailwind v4 with design tokens in `index.css`
- Lucide icons
- Light/dark theme toggle (persisted to localStorage)
- Monospace font for paths and file sizes
- 4-column grid of `MetricCard` components for summary numbers
- `SafetyBanner` with shield icon for trust/permission messaging
- `StatusBadge` with color tones: success, warning, neutral, purple
- `EmptyState` with dashed border and info icon for blank states

---

## 3. The Three Mole Workflows — Detailed

### Workflow A: Cleanup (`#/clean`)

This is the main Mole-powered page. It handles the full cleanup lifecycle.

#### Step-by-step user experience

1. **User arrives at `#/clean`.** The page shows a header ("Clean" / "Review Mole's current cleanup candidates") and a primary button.

2. **User clicks "Generate preview."** The frontend calls `transport.cleanupPreview()`. Rust runs `mo clean --dry-run`, which writes a human-readable text file to `~/.config/mole/clean-list.txt`. The adapter parses this file into structured categories and items. A loading state is shown during this (can take 10-30 seconds).

3. **Preview arrives.** The page shows:
   - **4 MetricCards** in a row: Potential Cleanup (total reclaimable bytes), Items (count), Categories (count), System Scope ("Full" / "User only" / "Unknown")
   - **Safety banners** for any warnings from Mole
   - **Paginated list** of items grouped by category (PAGE_SIZE = 50). Each category has a header. Each item shows: path (monospace), reported size, item count.
   - **Footer** with backend name, version, and generation timestamp
   - **"Review and run cleanup"** button — enabled only when preview has items > 0 and nothing is currently running

4. **User clicks "Review and run cleanup."** The frontend calls `transport.prepareCleanup()`. Rust creates a one-time confirmation token bound to the exact preview (version, item count, scope). A `CleanupPreparation` is returned with `expires_in_seconds` and a `warning` string.

5. **Confirmation dialog opens.** Shows: backend name, reviewed-at time, reviewed item count, reviewed size, system scope, confirmation expiry countdown. User must explicitly confirm.

6. **User confirms.** Frontend calls `transport.confirmCleanup(token)`. Rust runs `mo clean` (fresh discovery — Mole re-discovers eligible items). The UI transitions to a "running" state showing "Mole is cleaning this Mac" with a stop button.

7. **Execution completes.** A `CleanupOperation` is returned with: completion status (Finished/Partial/Cancelled/Failed), warnings, timestamp. The result is displayed as a card with a `StatusBadge`. The preview is dropped (ADR 0020 — a second run must review fresh discovery).

8. **Activity feed.** The operation is journaled in `#/activity` with: backend, version, reviewed items (NOT "removed" — Mole doesn't report per-path results), scope, completion, warnings.

#### State machine (`cleanup-flow.ts`)

```
States: preview → preparation → running → result
Events: previewStarted, previewArrived, previewFailed, reviewed, reviewFailed,
        runStarted, runFinished, runFailed, stopRequested, previewStopRequested
```

Key rules:

- `runStarted` drops the preview immediately — Rust has forgotten it, so the UI must not show paths describing the past
- `reviewFailed` clears both preparation AND preview (stale/empty preview)
- A stopped run still removed files — partial/cancelled/failed are all warnings, not errors
- `canReview(state)` = not previewing, not running, preview exists with items > 0

#### Data types

```typescript
// Preview — what Mole found
interface CleanupPreview {
  backend: string;
  backendVersion: string;
  generatedAt: string; // ISO timestamp
  categories: CleanupCategory[];
  potentialCleanup: number | null; // total reclaimable bytes
  totalItems: number;
  systemScope: "included" | "userOnly" | "unknown";
  warnings: string[];
}

interface CleanupCategory {
  name: string; // e.g. "Caches", "Logs"
  items: CleanupItem[];
}

interface CleanupItem {
  path: string; // absolute path
  reportedSize: number | null; // bytes
  itemCount: number; // files within this path
}

// Preparation — the confirmation token
interface CleanupPreparation {
  confirmationToken: number; // one-time token, expires
  backend: string;
  backendVersion: string;
  previewGeneratedAt: string;
  potentialCleanup: number | null;
  totalItems: number;
  systemScope: "included" | "userOnly" | "unknown";
  warnings: string[];
  expiresInSeconds: number;
  requiresConfirmation: boolean;
  warning: string; // "Mole will re-discover eligible candidates during execution..."
}

// Operation — the journal entry
interface CleanupOperation {
  id: number;
  backend: string;
  backendVersion: string;
  previewGeneratedAt: string;
  reviewedItems: number; // NOT "removed" — Mole doesn't report per-path
  reviewedPotentialCleanup: number | null;
  systemScope: "included" | "userOnly" | "unknown";
  completion: "finished" | "partial" | "cancelled" | "failed";
  warnings: string[];
  executedAtMs: number;
  logError: string | null;
}
```

---

### Workflow B: App Uninstall (`#/storage` → Applications view)

This workflow is embedded in the Storage page's Applications sub-view.

#### Step-by-step user experience

1. **User navigates to Storage → Applications.** The page fetches installed apps from `transport.installedApplicationInventory()` (which runs `mo uninstall --list`). If Mole is not available, it falls back to the scan tree's application section.

2. **Apps list displays.** Each app row shows: avatar initial (first letter of name), app name, path (monospace), source badge (e.g. "Homebrew cask"), uninstall name (what Mole accepts as an identifier), reported size. The list has search filtering and an optional largest-first sort.

3. **User clicks "Uninstall" on an app.** The frontend calls `transport.uninstallPreview([name])`. A loading state appears on that row ("Checking...").

4. **Preview arrives.** The `UninstallReview` dialog opens showing:
   - Title: "Uninstall {name}?"
   - Per-app section: name, Homebrew cask badge, reported size, list of items
   - Each item shows: `−` for removed items, `!` for reviewOnly items, scope tag ("system" or "left in place"), reported size
   - Warnings in a muted background
   - Notes: "backend will not handle these" — items the user must act on themselves
   - Expandable transcript: "Show/hide Mole's own output" — raw backend text in monospace

5. **User clicks "Continue."** Frontend calls `transport.prepareUninstall()`. Rust creates a one-time confirmation token. The dialog transitions to a two-step confirmation: the "Continue" button becomes a destructive "Move to Trash" button.

6. **User confirms with "Move to Trash."** Frontend calls `transport.confirmUninstall(token)`. Rust runs `mo uninstall <name>` with `y\n` on stdin. The dialog shows a running state.

7. **Execution completes.** `UninstallOperation` returned with: completion status, removed list, failed list, reported freed space, warnings, transcript. The dialog closes. The removed app's row stays in the list but is struck through (avoids renumbering under the virtualizer).

8. **Activity feed.** The operation is journaled with: backend, reviewed applications, reviewed items, completion, removed/failed lists, freed space, warnings.

#### State machine (`uninstall-flow.ts`)

```
States: idle → reviewing → prepared → running → result
Events: reviewStarted, reviewed, reviewFailed, prepared, dismissed,
        runStarted, removed, runFailed, inventoryReloaded
```

Key rules:

- Request identity via `requestId` — late replies from abandoned requests are silently dropped
- `removedNames` accumulates across the session (names are stable, node IDs are not)
- `prepared` keeps the plan on screen beside its confirmation (replacing with summary at approval time would ask user to agree to something they can no longer read)
- `runStarted` drops both preview and preparation (token is spent, plan is about to stop being true)
- `removed` marks from what the backend _reported_, not what was asked (partial runs)
- `inventoryReloaded` preserves `removedNames` and `last` when not running; resets everything else

#### Data types

```typescript
// Installed app from Mole
interface InstalledApplication {
  name: string; // display name
  bundleId: string; // e.g. "com.example.desktop"
  source: string; // "App", "Homebrew", etc.
  uninstallName: string; // the identifier Mole's uninstall command accepts
  path: string; // absolute path to .app bundle
  reportedSize: string; // "410.9MB" — text, not bytes
}

// Preview — what Mole would remove
interface UninstallPreview {
  backend: string;
  backendVersion: string;
  requested: string[]; // identifiers requested
  apps: UninstallApp[];
  reportedTotal: string | null;
  totalItems: number;
  hasReviewOnlyItems: boolean;
  warnings: string[];
  notes: string[];
  transcript: string; // ANSI-stripped verbatim backend output
}

interface UninstallApp {
  name: string;
  homebrewCask: boolean;
  reportedSize: string | null;
  items: UninstallItem[];
}

interface UninstallItem {
  displayPath: string; // tilde-abbreviated, NOT resolvable
  reportedSize: string | null;
  scope: "removed" | "system" | "reviewOnly";
}

// Preparation — the confirmation token
interface UninstallPreparation {
  confirmationToken: number;
  backend: string;
  backendVersion: string;
  applications: string[]; // display names for the dialog sentence
  reportedTotal: string | null;
  totalItems: number;
  hasReviewOnlyItems: boolean;
  warnings: string[];
  expiresInSeconds: number;
  requiresConfirmation: boolean;
  warning: string; // "X and the files listed above will be moved to the Trash by Y."
}

// Operation — the journal entry
interface UninstallOperation {
  id: number;
  backend: string;
  backendVersion: string;
  reviewedApplications: string[];
  reviewedItems: number;
  reviewedTotal: string | null;
  completion: "finished" | "partial" | "cancelled" | "failed";
  removed: string[];
  failed: string[];
  reportedFreed: string | null;
  warnings: string[];
  executedAtMs: number;
  logError: string | null;
}
```

---

### Workflow C: System Status (`#/storage` → System section)

This is a read-only display, not an interactive workflow.

#### User experience

1. **Section is collapsed by default.** It shows a chevron, "System status" label, and when loaded, an inline health score and Mac model.

2. **User expands the section.** On first open, `transport.systemStatus()` is called (runs `mo status --json`). A refresh button appears when open.

3. **Content displays:**
   - **4 MetricCards:** Health (score/100), CPU (usage %, logical cores), Memory (used %, available, pressure), Uptime (with model)
   - **Hardware section:** Mac model, Processor, Memory, Storage, macOS version, Host
   - **Disks section:** mount point, used%, used/total bytes, filesystem type, external flag, SMART status
   - **Power & Temperature:** batteries (percent, status, health), CPU temp, GPU temp, fan speed
   - **Footer:** "Snapshot from {backend} at {time}. Nothing leaves this Mac."

#### Data type

```typescript
interface SystemStatus {
  backend: string;
  collectedAt: string;
  host: string;
  platform: string;
  uptime: string;
  healthScore: number; // 0-100
  healthScoreMsg: string;
  hardware: HardwareStatus;
  cpu: CpuStatus;
  memory: MemoryStatus;
  disks: DiskStatus[];
  batteries: BatteryStatus[];
  thermal: ThermalStatus;
}

interface HardwareStatus {
  model: string; // e.g. "MacBook Pro (14-inch, 2023)"
  cpuModel: string;
  totalRam: number; // bytes
  diskSize: number;
  osVersion: string;
}

interface CpuStatus {
  usage: number; // percent
  load1: number;
  load5: number;
  load15: number;
  coreCount: number;
  logicalCpu: number;
}

interface MemoryStatus {
  used: number;
  total: number;
  available: number;
  swap: number;
  swapTotal: number;
  usedPercent: number;
  pressure: string; // "Normal", "Warn", etc.
}

interface DiskStatus {
  mount: string;
  device: string;
  used: number;
  total: number;
  fstype: string;
  external: boolean;
  smartStatus: string;
}

interface BatteryStatus {
  percent: number;
  status: string; // "Charging", "Discharging", etc.
  timeLeft: string | null;
  health: string;
  cycleCount: number;
  capacity: number;
}

interface ThermalStatus {
  cpuTemp: number | null;
  gpuTemp: number | null;
  batteryTemp: number | null;
  fanSpeed: number | null;
  fanCount: number | null;
}
```

---

## 4. Existing UX Patterns (for consistency)

These patterns are already established in the codebase. New Mole GUI work should follow them.

### Confirmation pattern (two-step)

1. User triggers an action → backend returns a preparation with a one-time token and expiry
2. Dialog shows the full plan (what will happen, what the backend decided)
3. User explicitly confirms → token is consumed, backend executes
4. Token cannot be reused. Single-click approval is explicitly prevented.

### Safety banner pattern

- Shield icon + message + optional "Learn More" link
- Default text: "Nothing is removed without your confirmation"
- Cleanup page: "Mole decides what is removed" — emphasizes Nirmoka doesn't copy protection rules
- Varies by context (installed apps vs scan-derived apps)

### Status badge pattern

- Colored badge: `success` (green), `warning` (yellow), `neutral` (gray), `purple`
- Used for completion states: "Finished" = success, "Partial"/"Cancelled"/"Failed" = warning

### Struck-through row pattern

- After an item is removed/trashed, its row stays in the list with strikethrough text
- Avoids renumbering under the virtualizer (which would shift scroll position)
- The row is visually dead but still occupies space

### Virtualized list pattern

- TanStack Virtual with 36px row height, 12-row overscan
- `listbox` with `aria-activedescendant` (not per-row focus)
- Keyboard: arrows, Page Up/Down, Home/End, Enter to open, Backspace to go up

### Metric card pattern

- 4-column grid of cards showing: label (small text), value (large number), hint (small description)
- Used for summary stats: total items, reclaimable space, health score, etc.

### Empty state pattern

- Dashed border, centered Info icon, title + description
- Used when no data is available (no scan, no preview, no results)

### Skeleton loading pattern

- Placeholder rectangles matching the shape of real content
- Shown during async data fetches

### Request identity pattern

- Every async operation gets a monotonically increasing ID
- Late replies from abandoned requests are silently dropped
- Prevents stale data from overwriting fresh state

---

## 5. Backend Gating

The app detects which backends are available and adjusts the UI accordingly:

- `cleanupAvailability(backends)` — checks Mole is usable AND has `cleanupCategories` + `dryRun` capabilities. Returns `{ available: boolean, reason: string }`.
- `uninstallOffer(backends)` — returns `"app"` (full preview+remove), `"terminal"` (list only, user runs command manually), or `"none"` (no inventory).
- `scanAvailability(backends, selection)` — whether scanning is possible.

The `reason` string is always human-readable and shown to the user when a feature is unavailable.

---

## 6. Safety Constraints That Affect UX

These are non-negotiable. The GUI must respect them.

1. **Mole owns the protection rules.** Never tell the user "this is safe" — tell them "Mole says this is eligible for removal."

2. **Preview ≠ execution.** The cleanup preview shows what Mole found at time T. By the time the user confirms, the state may have changed. Mole re-discovers on execution. The UI must not promise that the execution will match the preview exactly.

3. **One-time confirmation tokens.** Every destructive operation requires: prepare → review → confirm with token. Tokens expire. No bypass.

4. **Transcript is always available.** The raw backend output is kept alongside the parsed form. Users can always see exactly what Mole said, not just Nirmoka's interpretation.

5. **No permanent deletion from GUI.** Uninstall routes to Trash by default. The `--permanent` flag is never passed.

6. **Partial success is the norm.** Mole can remove some items and fail on others. The UI must present this as "completed with warnings," not as an error.

7. **Cancelled ≠ failed.** Cancellation is a user decision. A cancelled run may have already removed some files. Both are warnings, not errors.

8. **Paths from Rust's validator.** Confirmation dialogs show the resolved path from Rust, not the row name the user clicked. This prevents confusion when symlinks or aliases are involved.

---

## 7. Specific UX Questions

Please address each of these in your response:

### Q1: Cleanup preview layout

The cleanup preview groups items by category (Caches, Logs, etc.). Currently it's a paginated flat list with category headers. Should it be:

- (a) Collapsible sections per category (accordion)?
- (b) Flat list with sticky category headers?
- (c) Cards per category with a summary, expandable to items?
- Something else?

Consider: there can be 5-10 categories with 10-100 items each. The user needs to understand the big picture (how much space, what kinds of cleanup) before diving into items.

### Q2: Cleanup preview ≠ execution transparency

Mole re-discovers on execution. The preview is a snapshot. How should the UI communicate this without undermining trust? Options:

- (a) Prominent banner: "Mole will re-check everything when cleaning"
- (b) Subtle footnote in the confirmation dialog
- (c) Both — banner on preview page, reminder in confirmation
- How much detail about _why_ this happens should we share?

### Q3: Uninstall review dialog structure

The uninstall preview shows per-app items with scopes (Removed/System/ReviewOnly). Currently it's a single dialog with all apps and their items. For multi-app uninstalls, should it be:

- (a) One dialog with all apps, each in a collapsible section?
- (b) One dialog per app, reviewed sequentially?
- (c) Summary card per app, click to expand for details?

How should "system" scope items be visually distinguished? They're items Mole will leave in place — the user can't remove them through the GUI.

### Q4: Transcript presentation

Mole's raw output is human-readable text with ANSI codes (stripped). It's the ground truth. How prominent should it be?

- (a) Hidden behind "Show advanced details" toggle (current approach)
- (b) Always visible in a scrollable panel
- (c) Collapsible section at the bottom of the dialog
- Should it be in a `<pre>` block, or styled differently?

### Q5: System status prominence

Health score is 0-100. Currently it's one of 4 MetricCards in a collapsible section. Should it be:

- (a) More prominent — hero card at the top of the section?
- (b) Keep as-is — one metric among many?
- (c) Visual indicator in the sidebar or header (like a badge)?
- How should the health score be color-coded? Thresholds?

### Q6: Progress feedback

Mole commands can take 10-30 seconds. Mole outputs human-readable text, not structured progress events. The adapter can only report "running" or "not running." How to show progress?

- (a) Indeterminate spinner (current approach)
- (b) Elapsed time counter
- (c) "Mole is working..." with a subtle animation
- Should dry-runs and executions be treated differently?

### Q7: Partial success presentation

Mole can partially succeed (some items removed, some not). The `Completion` enum is: Finished, Partial, Cancelled, Failed. How should each state be presented?

- "Partial" — show what was removed? Show what failed? Both?
- "Cancelled" — the user stopped it. Some files may have been removed. How to communicate this?
- "Failed" — nothing was removed (pre-spawn failure). How to distinguish from Partial?

### Q8: Cross-workflow context

Clean and Activity are separate destinations. After a cleanup run, the user might want to see the result in Activity. Should there be:

- (a) A link/button from the cleanup result to Activity?
- (b) A toast/notification that appears briefly?
- (c) Both?
- How much context should Activity show about each operation?

### Q9: Safety messaging tone

The app currently says "Nothing is removed without your confirmation" and "Mole decides what is removed." How should this scale to the uninstall workflow? The user is uninstalling entire applications, not just cleaning caches. Options:

- (a) Same tone — trust the backend, user confirms
- (b) More cautious — "This will remove {app} and {N} associated files"
- (c) Context-dependent — more detail for uninstall, less for cleanup
- How to avoid "permission dialog fatigue" where users just click through?

### Q10: Empty and error states

What should the user see when:

- Mole is not installed? (backend not found)
- Mole is an unsupported version?
- Mole is installed but the `mo` command fails?
- The cleanup preview returns zero items?
- The uninstall preview returns an error for one app in a multi-app batch?
