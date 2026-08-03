/**
 * The transport boundary.
 *
 * # Why this module exists
 *
 * This is the ONLY place in the frontend allowed to import `@tauri-apps/*` or
 * otherwise know how messages reach the backend. Every component imports from
 * here instead.
 *
 * If components call `invoke()` directly, the React code is welded to Tauri and
 * the "we could leave Tauri" claim becomes fiction. With this boundary, moving
 * to Electron rewrites one file; moving to a browser-served build rewrites one
 * file. CI greps for `@tauri-apps` imports outside this package and fails.
 *
 * @see docs/adr/0005-frontend-port.md
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  Backend,
  BackendSelection,
  ApplicationInventory,
  Capabilities,
  CleanupOperation,
  CleanupPreparation,
  CleanupPreview,
  DeleteOperation,
  DeletePreparation,
  PlatformFeatures,
  Row,
  RowPage,
  DeveloperInventory,
  InstalledApplicationInventory,
  ScanFailure,
  ScanProgress,
  ScanSummary,
  Sort,
  SystemStatus,
  VolumeInfo,
} from "./types.js";

export type * from "./types.js";

export type Unsubscribe = () => void;

/**
 * Event names, shared with `crates/app/src/scan.rs`. A typo in one is a UI that
 * silently never updates, so they are written down once.
 */
const EVENT = {
  progress: "scan://progress",
  finished: "scan://finished",
  failed: "scan://failed",
} as const;

/**
 * What the frontend needs from a backend, stated without reference to how it
 * gets there.
 */
export interface Transport {
  /** Which disk backends are installed, and which of them are usable. */
  listBackends(): Promise<Backend[]>;

  /** Capabilities of the backend that runs scans, for hiding controls. */
  capabilities(): Promise<Capabilities>;

  /**
   * Which backend the user picked, and which one will actually scan.
   *
   * Both, because they differ: Mole is the macOS default and cannot scan, so
   * choosing it leaves ncdu scanning. `scannerInsteadOf` names who was asked
   * for, which is what keeps a fallback from reading as a setting being
   * ignored.
   */
  backendSelection(): Promise<BackendSelection>;

  /**
   * Pick a backend, or pass `null` to go back to the platform default.
   *
   * Resolves with the selection as it now stands rather than echoing the id:
   * a choice is honoured where it can be, and the caller needs to know what it
   * resolved to rather than what was requested.
   *
   * Does not reject on a failed write. The choice takes effect in memory before
   * it is persisted, so a rejection would leave this window showing the old
   * backend while the process used the new one. A write that failed comes back
   * as `saveError` on the selection it did not prevent.
   */
  chooseBackend(id: string | null): Promise<BackendSelection>;

  /**
   * Begin a scan. Resolves with the canonical root actually being scanned,
   * which differs from the argument for a relative path or a symlink.
   *
   * Everything after that arrives as events: this returns when the worker
   * thread starts, not when the scan finishes.
   */
  startScan(rootPath: string): Promise<string>;

  /** Stop the running scan, killing the backend process. */
  cancelScan(): Promise<boolean>;

  /** Totals for the last completed scan, or null if none has completed. */
  scanSummary(): Promise<ScanSummary | null>;

  /**
   * One window of one directory's children, largest first. A `parentId` of
   * `null` asks for the scan root.
   *
   * `scanId` comes from the summary or the page the `parentId` was read from.
   * Both are required because a node id alone means nothing: every scan numbers
   * its tree from zero, so an id kept across a rescan would quietly name a
   * different directory. Passing the id of a scan that has been replaced is an
   * error, not a wrong answer.
   *
   * `sort` orders the whole directory before the window is cut, which is why it
   * is a request parameter rather than something the caller applies to the rows
   * it received. Sorting a window client-side would order the forty rows on
   * screen and leave the other hundred thousand where they were.
   *
   * The tree lives in Rust. Never request the whole tree — a home directory can
   * be millions of nodes, and rendering that as DOM is the mistake that gets
   * blamed on the GUI framework. The Rust side caps `limit` whatever is asked
   * for.
   */
  rows(
    scanId: number,
    parentId: number | null,
    sort: Sort,
    offset: number,
    limit: number,
  ): Promise<RowPage>;

  /** Validate a scanned row and receive a one-time confirmation token. */
  prepareDelete(scanId: number, nodeId: number): Promise<DeletePreparation>;

  /** Execute the exact adapter plan held behind a one-time token. */
  confirmDelete(confirmationToken: number): Promise<DeleteOperation>;

  /** Restore one recoverable deletion through its original backend. */
  undoDelete(operationId: number): Promise<DeleteOperation>;

  /** Newest-first durable deletion journal. */
  operationLog(): Promise<DeleteOperation[]>;

  /** Real filesystem capacity for the volume containing `path` (macOS beta). */
  volumeInfo(path: string): Promise<VolumeInfo>;

  /** Application bundles evidenced by the completed Rust-side scan tree. */
  applicationInventory(scanId: number): Promise<ApplicationInventory>;

  /** Backend-produced applications with stable uninstall identifiers. */
  installedApplicationInventory(): Promise<InstalledApplicationInventory>;

  /** Developer data evidenced by the completed Rust-side scan tree. */
  developerInventory(scanId: number): Promise<DeveloperInventory>;

  /** One backend-produced system health snapshot. */
  systemStatus(): Promise<SystemStatus>;

  /** Fresh backend-owned cleanup discovery. Never removes anything. */
  cleanupPreview(): Promise<CleanupPreview>;

  /** Cancel the active cleanup discovery, if one is running. */
  cancelCleanupPreview(): Promise<boolean>;

  /** Bind latest Rust-held cleanup preview to a short-lived one-time token. */
  prepareCleanup(): Promise<CleanupPreparation>;

  /**
   * Spend one confirmation token and run the backend's own cleanup.
   *
   * The token carries no paths. Mole re-discovers candidates itself, so what
   * comes back describes the run — scope, completion, backend warnings — rather
   * than the reviewed rows.
   */
  confirmCleanup(confirmationToken: number): Promise<CleanupOperation>;

  /** Cancel the active cleanup run. What it already removed stays removed. */
  cancelCleanup(): Promise<boolean>;

  /** Durable cleanup journal, newest first, reloaded across launches. */
  cleanupLog(): Promise<CleanupOperation[]>;

  /**
   * What this desktop can do with a selected path, and what to call it.
   *
   * Not a backend capability: revealing a file involves no disk tool. The label
   * travels with the flag because every platform names the action differently.
   */
  platformFeatures(): Promise<PlatformFeatures>;

  /**
   * Open the platform's file manager with this row selected.
   *
   * Takes the scan-and-node pair, not a path: Rust resolves the path from the
   * tree that issued the id, so the frontend never assembles one.
   */
  revealInFileManager(scanId: number, nodeId: number): Promise<void>;

  /** Show this row in Quick Look. Rejects where the platform has none. */
  quickLook(scanId: number, nodeId: number): Promise<void>;

  /**
   * Subscriptions resolve when the listener is REGISTERED, not when an event
   * arrives.
   *
   * Registering is a round trip into Rust, and a scan started before it
   * completes can finish before anyone is listening — the terminal event lands
   * with no subscriber and the window sits on "scanning" forever. Callers must
   * not start a scan until these have resolved; `ScanPanel` keeps its button
   * disabled until then.
   */
  onScanProgress(handler: (progress: ScanProgress) => void): Promise<Unsubscribe>;
  onScanFinished(handler: (summary: ScanSummary) => void): Promise<Unsubscribe>;
  onScanFailed(handler: (failure: ScanFailure) => void): Promise<Unsubscribe>;
}

/**
 * Registers a listener and resolves with its cleanup.
 *
 * The returned unsubscribe is safe to call before registration finishes, which
 * is the case that actually happens: StrictMode mounting and unmounting an
 * effect faster than the round trip completes.
 */
async function subscribe<T>(event: string, handler: (payload: T) => void): Promise<Unsubscribe> {
  const unlisten = await listen<T>(event, (message) => handler(message.payload));
  return () => unlisten();
}

/** The real thing: every call is a command in `crates/app`. */
export function tauriTransport(): Transport {
  return {
    listBackends: () => invoke<Backend[]>("list_backends"),
    capabilities: () => invoke<Capabilities>("capabilities"),
    backendSelection: () => invoke<BackendSelection>("backend_selection"),
    chooseBackend: (id) => invoke<BackendSelection>("choose_backend", { id }),
    startScan: (rootPath) => invoke<string>("start_scan", { rootPath }),
    cancelScan: () => invoke<boolean>("cancel_scan"),
    scanSummary: () => invoke<ScanSummary | null>("scan_summary"),
    rows: (scanId, parentId, sort, offset, limit) =>
      invoke<RowPage>("rows", { scanId, parentId, sort, offset, limit }),
    prepareDelete: (scanId, nodeId) =>
      invoke<DeletePreparation>("prepare_delete", { scanId, nodeId }),
    confirmDelete: (confirmationToken) =>
      invoke<DeleteOperation>("confirm_delete", { confirmationToken }),
    undoDelete: (operationId) => invoke<DeleteOperation>("undo_delete", { operationId }),
    operationLog: () => invoke<DeleteOperation[]>("operation_log"),
    volumeInfo: (path) => invoke<VolumeInfo>("volume_info", { path }),
    applicationInventory: (scanId) =>
      invoke<ApplicationInventory>("application_inventory", { scanId }),
    installedApplicationInventory: () =>
      invoke<InstalledApplicationInventory>("installed_application_inventory"),
    developerInventory: (scanId) => invoke<DeveloperInventory>("developer_inventory", { scanId }),
    systemStatus: () => invoke<SystemStatus>("system_status"),
    cleanupPreview: () => invoke<CleanupPreview>("cleanup_preview"),
    cancelCleanupPreview: () => invoke<boolean>("cancel_cleanup_preview"),
    prepareCleanup: () => invoke<CleanupPreparation>("prepare_cleanup"),
    confirmCleanup: (confirmationToken) =>
      invoke<CleanupOperation>("confirm_cleanup", { confirmationToken }),
    cancelCleanup: () => invoke<boolean>("cancel_cleanup"),
    cleanupLog: () => invoke<CleanupOperation[]>("cleanup_log"),
    platformFeatures: () => invoke<PlatformFeatures>("platform_features"),
    revealInFileManager: (scanId, nodeId) =>
      invoke<void>("reveal_in_file_manager", { scanId, nodeId }),
    quickLook: (scanId, nodeId) => invoke<void>("quick_look", { scanId, nodeId }),

    onScanProgress: (handler) => subscribe(EVENT.progress, handler),
    onScanFinished: (handler) => subscribe(EVENT.finished, handler),
    onScanFailed: (handler) => subscribe(EVENT.failed, handler),
  };
}

/** Whether this build is running inside the Tauri shell. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * The transport for wherever this is running.
 *
 * Inside the shell, the real one. In a plain browser — `pnpm dev` on its own, or
 * a component test — the mock, so the UI can be worked on without a backend or a
 * Rust toolchain. The alternative is a blank screen and `invoke is not defined`
 * in a console nobody has open.
 */
export function resolveTransport(): Transport {
  return isTauri() ? tauriTransport() : createMockTransport();
}

/** One node of the mock tree. Mirrors what the Rust arena holds per node. */
interface MockNode {
  id: number;
  name: string;
  kind: Row["kind"];
  bytes: number;
  readError?: boolean;
  children: number[];
}

/**
 * A small tree with the shapes the UI has to survive: a directory big enough to
 * need virtualizing, one that cannot be read, and one that is genuinely empty.
 *
 * Sizes are round fixture numbers rather than plausible ones, because a mock
 * that looks like a real scan is a mock somebody eventually mistakes for one.
 */
function mockTree(): Map<number, MockNode> {
  const nodes: MockNode[] = [
    { id: 0, name: "root", kind: "directory", bytes: 0, children: [1, 2, 3, 4] },
    { id: 1, name: "big", kind: "file", bytes: 3 * 1024 * 1024, children: [] },
    { id: 2, name: "many", kind: "directory", bytes: 0, children: [] },
    { id: 3, name: "denied", kind: "directory", bytes: 0, readError: true, children: [] },
    { id: 4, name: "empty", kind: "directory", bytes: 0, children: [] },
  ];

  // Enough to scroll: a list this long is the reason the window exists.
  const many = nodes.find((node) => node.name === "many")!;
  for (let index = 0; index < 500; index += 1) {
    const id = 100 + index;
    many.children.push(id);
    nodes.push({
      id,
      name: `entry-${String(index).padStart(3, "0")}`,
      kind: "file",
      bytes: (index + 1) * 1024,
      children: [],
    });
  }

  return new Map(nodes.map((node) => [node.id, node]));
}

/**
 * In-memory Transport for UI development and tests.
 *
 * It reports a plausible ncdu and a fixture tree, and it does not pretend to
 * walk a disk: `startScan` completes immediately with numbers that are visibly
 * a fixture.
 */
export function createMockTransport(overrides: Partial<Transport> = {}): Transport {
  const nodes = mockTree();

  /** Same bottom-up pass as `Tree::rollup`, so directory sizes are not invented. */
  const totalOf = (id: number): number => {
    const node = nodes.get(id);
    if (!node) return 0;
    return node.children.reduce((sum, child) => sum + totalOf(child), node.bytes);
  };

  const parents = new Map<number, number>();
  for (const node of nodes.values()) {
    for (const child of node.children) parents.set(child, node.id);
  }

  const summary: ScanSummary = {
    scanId: 1,
    rootId: 0,
    rootPath: "/fixtures/root",
    totalBytes: totalOf(0),
    entries: nodes.size,
    directories: [...nodes.values()].filter((node) => node.kind === "directory").length,
    backendId: "ncdu",
    backendVersion: "2.8.2",
    readErrors: 1,
    excluded: 0,
    hardlinksDeduplicated: 0,
    hardlinkBytesSaved: 0,
  };

  let onFinished: ((summary: ScanSummary) => void) | null = null;

  /**
   * The mock reports a macOS pair, so it uses the macOS default order.
   *
   * Resolution mirrors `Registry::resolve`: the choice first, then this order,
   * and at every step only among backends that can do the job. A mock that
   * simply echoed the chosen id would render a state the real app cannot reach —
   * "Mole is scanning" — and the UI built against it would be wrong.
   */
  const DEFAULT_ORDER = ["mole", "rip", "ncdu", "gdu"];
  let chosen: string | null = null;

  const selection = async (): Promise<BackendSelection> => {
    const scanners = (await base.listBackends())
      .filter((backend) => backend.usable && backend.capabilities.scan)
      .map((backend) => backend.id);

    const scanner =
      (chosen !== null && scanners.includes(chosen) ? chosen : null) ??
      DEFAULT_ORDER.find((id) => scanners.includes(id)) ??
      scanners[0] ??
      null;

    return {
      chosen,
      defaultOrder: DEFAULT_ORDER,
      scanner,
      // Only when a choice was made and something else ran.
      scannerInsteadOf: chosen !== null && chosen !== scanner ? chosen : null,
      persistent: true,
      saveError: null,
    };
  };

  const base: Transport = {
    async listBackends() {
      return [
        {
          id: "ncdu",
          displayName: "ncdu",
          supportedVersions: ">=2.0, <3.0",
          detection: { state: "found", path: "/usr/local/bin/ncdu", version: "2.8.2" },
          error: null,
          usable: true,
          capabilities: {
            scan: true,
            delete: false,
            trash: false,
            undo: false,
            dryRun: false,
            cleanupCategories: false,
            appInventory: false,
            uninstallApps: false,
            systemStatus: false,
          },
        },
        // A usable backend that cannot scan. Present in the mock because it is
        // the case the UI gets wrong: without it, `pnpm dev` never renders the
        // state where a detected backend does not drive the browser.
        {
          id: "mole",
          displayName: "Mole",
          supportedVersions: ">=1.48, <2.0",
          detection: { state: "found", path: "/opt/homebrew/bin/mo", version: "1.48.1" },
          error: null,
          usable: true,
          capabilities: {
            scan: false,
            delete: false,
            trash: false,
            undo: false,
            dryRun: true,
            cleanupCategories: true,
            appInventory: true,
            uninstallApps: false,
            systemStatus: true,
          },
        },
        {
          id: "gdu",
          displayName: "gdu",
          supportedVersions: ">=5.32, <5.33",
          detection: { state: "notInstalled" },
          error: null,
          usable: false,
          capabilities: {
            scan: true,
            delete: false,
            trash: false,
            undo: false,
            dryRun: false,
            cleanupCategories: false,
            appInventory: false,
            uninstallApps: false,
            systemStatus: false,
          },
        },
        {
          id: "rip",
          displayName: "rip",
          supportedVersions: ">=0.13, <0.14",
          detection: { state: "notInstalled" },
          error: null,
          usable: false,
          capabilities: {
            scan: false,
            delete: true,
            trash: true,
            undo: true,
            dryRun: false,
            cleanupCategories: false,
            appInventory: false,
            uninstallApps: false,
            systemStatus: false,
          },
        },
      ];
    },

    async capabilities() {
      return {
        scan: true,
        delete: false,
        trash: false,
        undo: false,
        dryRun: false,
        cleanupCategories: false,
        appInventory: false,
        uninstallApps: false,
        systemStatus: false,
      };
    },

    backendSelection: selection,

    async chooseBackend(id) {
      chosen = id;
      return selection();
    },

    async startScan(rootPath) {
      queueMicrotask(() => onFinished?.({ ...summary, rootPath }));
      return rootPath;
    },

    async cancelScan() {
      return false;
    },

    async scanSummary() {
      return summary;
    },

    async rows(scanId, parentId, sort, offset, limit) {
      if (scanId !== summary.scanId) {
        throw new Error(`scan ${scanId} has been replaced by scan ${summary.scanId}`);
      }

      const parent = nodes.get(parentId ?? summary.rootId);
      if (!parent) throw new Error(`unknown node ${parentId}`);

      const parentTotal = totalOf(parent.id);
      const children = parent.children
        .map((id) => nodes.get(id))
        .filter((node): node is MockNode => node !== undefined)
        .sort((a, b) => {
          switch (sort) {
            case "largestFirst":
              return totalOf(b.id) - totalOf(a.id) || a.name.localeCompare(b.name);
            case "smallestFirst":
              return totalOf(a.id) - totalOf(b.id) || a.name.localeCompare(b.name);
            case "nameAscending":
              return a.name.localeCompare(b.name);
            case "nameDescending":
              return b.name.localeCompare(a.name);
          }
        });

      const ancestors = [];
      for (let cursor = parents.get(parent.id); cursor !== undefined;) {
        const node = nodes.get(cursor);
        if (!node) break;
        ancestors.unshift({ id: node.id, name: node.name });
        cursor = parents.get(node.id);
      }

      const path = [...ancestors.map((crumb) => crumb.name), parent.name]
        .slice(1)
        .reduce((joined, segment) => `${joined}/${segment}`, summary.rootPath);

      return {
        scanId,
        parentId: parent.id,
        name: parent.name,
        path,
        ancestors,
        readError: parent.readError ?? false,
        sort,
        offset,
        total: children.length,
        rows: children.slice(offset, offset + limit).map((node) => {
          const total = totalOf(node.id);
          return {
            id: node.id,
            name: node.name,
            kind: node.kind,
            ownBytes: node.bytes,
            apparentBytes: node.bytes,
            totalBytes: total,
            readError: node.readError ?? false,
            hardlink: false,
            excluded: false,
            childCount: node.children.length,
            share: parentTotal === 0 ? 0 : total / parentTotal,
          };
        }),
      };
    },

    async prepareDelete() {
      throw new Error("mock transport never performs destructive filesystem operations");
    },

    async confirmDelete() {
      throw new Error("mock transport never performs destructive filesystem operations");
    },

    async undoDelete() {
      throw new Error("mock transport has no deletion to undo");
    },

    async operationLog() {
      return [];
    },

    async volumeInfo() {
      return {
        mountPoint: "/fixtures",
        totalBytes: summary.totalBytes,
        usedBytes: summary.totalBytes,
        freeBytes: 0,
      };
    },

    async applicationInventory() {
      return { scanId: summary.scanId, total: 0, rows: [] };
    },

    async installedApplicationInventory() {
      return {
        backend: "mole",
        backendInsteadOf: null,
        total: 2,
        rows: [
          {
            name: "Example",
            bundleId: "com.example.desktop",
            source: "system",
            uninstallName: "Example",
            path: "/Applications/Example.app",
            totalBytes: 256 * 1024 ** 2,
          },
          {
            name: "Sample Tool",
            bundleId: "org.example.sample-tool",
            source: "user",
            uninstallName: "Sample Tool",
            path: "/Users/fixture/Applications/Sample Tool.app",
            totalBytes: 64 * 1024 ** 2,
          },
        ],
      };
    },

    async developerInventory() {
      return { scanId: summary.scanId, total: 0, rows: [] };
    },

    async systemStatus() {
      return {
        backend: "mole",
        backendInsteadOf: null,
        collectedAt: "2026-08-01T12:00:00Z",
        host: "fixture-mac",
        platform: "darwin/arm64",
        uptime: "2d 3h",
        healthScore: 91,
        healthScoreMessage: "Healthy fixture",
        hardware: {
          model: "Fixture Mac",
          cpuModel: "Fixture CPU",
          totalRam: "16 GB",
          diskSize: "512 GB",
          osVersion: "26.5",
        },
        cpu: {
          usage: 23.5,
          load1: 1.1,
          load5: 1,
          load15: 0.9,
          coreCount: 8,
          logicalCpu: 8,
        },
        memory: {
          used: 8 * 1024 ** 3,
          total: 16 * 1024 ** 3,
          available: 8 * 1024 ** 3,
          usedPercent: 50,
          swapUsed: 0,
          swapTotal: 4 * 1024 ** 3,
          pressure: "normal",
        },
        disks: [
          {
            mount: "/",
            device: "/dev/fixture",
            used: 200 * 1024 ** 3,
            total: 500 * 1024 ** 3,
            usedPercent: 40,
            filesystem: "apfs",
            external: false,
            smartStatus: "verified",
          },
        ],
        batteries: [
          {
            percent: 82,
            status: "discharging",
            timeLeft: "5h 10m",
            health: "normal",
            cycleCount: 120,
            capacity: 94,
          },
        ],
        thermal: {
          cpuTemp: 48,
          gpuTemp: 44,
          batteryTemp: 31,
          fanSpeed: 0,
          fanCount: 1,
        },
      };
    },

    async cleanupPreview() {
      return {
        backend: "mole",
        backendInsteadOf: null,
        backendVersion: "1.48.1",
        generatedAt: "2026-08-01 12:30:00",
        potentialCleanup: "At least 192.00MB",
        totalItems: 6,
        systemScope: "userOnly",
        warnings: [
          "System-level candidates are not included because administrator access was unavailable.",
        ],
        categories: [
          {
            name: "Browser caches",
            items: [
              {
                path: "/Users/fixture/Library/Caches/com.example.browser",
                reportedSize: "128.00MB",
                itemCount: 4,
              },
              {
                path: "/Users/fixture/Library/Caches/com.example.helper",
                reportedSize: "64.00MB",
                itemCount: 1,
              },
            ],
          },
          {
            name: "Developer caches",
            items: [
              {
                path: "/Users/fixture/Library/Developer/Example",
                reportedSize: null,
                itemCount: 1,
              },
            ],
          },
        ],
      };
    },

    async cancelCleanupPreview() {
      return false;
    },

    async prepareCleanup() {
      throw new Error("mock transport never prepares destructive cleanup operations");
    },

    async confirmCleanup() {
      throw new Error("mock transport never runs destructive cleanup operations");
    },

    async cancelCleanup() {
      return false;
    },

    async cleanupLog() {
      return [];
    },

    // Outside the shell there is no desktop to hand a path to, so the mock says
    // what it can do rather than pretending the buttons work.
    async platformFeatures() {
      return { revealLabel: "Reveal in Finder", quickLook: false };
    },

    async revealInFileManager() {
      throw new Error("mock transport cannot reach a file manager");
    },

    async quickLook() {
      throw new Error("mock transport cannot open Quick Look");
    },

    async onScanProgress() {
      return () => {};
    },

    async onScanFinished(handler) {
      onFinished = handler;
      return () => {
        onFinished = null;
      };
    },

    async onScanFailed() {
      return () => {};
    },
  };

  return { ...base, ...overrides };
}
