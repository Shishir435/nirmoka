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
  Capabilities,
  RowPage,
  ScanFailure,
  ScanProgress,
  ScanSummary,
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

  /** Capabilities of the active backend, for hiding unsupported controls. */
  capabilities(): Promise<Capabilities>;

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
   * The tree lives in Rust. Never request the whole tree — a home directory can
   * be millions of nodes, and rendering that as DOM is the mistake that gets
   * blamed on the GUI framework. The Rust side caps `limit` whatever is asked
   * for.
   */
  rows(parentId: number | null, offset: number, limit: number): Promise<RowPage>;

  onScanProgress(handler: (progress: ScanProgress) => void): Unsubscribe;
  onScanFinished(handler: (summary: ScanSummary) => void): Unsubscribe;
  onScanFailed(handler: (failure: ScanFailure) => void): Unsubscribe;
}

/**
 * Tauri's `listen` is async, but a React effect must hand back its cleanup
 * synchronously. This bridges the two, including the case that actually
 * happens: StrictMode unsubscribing before the listener has been registered.
 */
function subscribe<T>(event: string, handler: (payload: T) => void): Unsubscribe {
  let cancelled = false;

  const pending = listen<T>(event, (message) => handler(message.payload)).then((unlisten) => {
    if (cancelled) unlisten();
    return unlisten;
  });

  return () => {
    cancelled = true;
    void pending.then((unlisten) => unlisten()).catch(() => {});
  };
}

/** The real thing: every call is a command in `crates/app`. */
export function tauriTransport(): Transport {
  return {
    listBackends: () => invoke<Backend[]>("list_backends"),
    capabilities: () => invoke<Capabilities>("capabilities"),
    startScan: (rootPath) => invoke<string>("start_scan", { rootPath }),
    cancelScan: () => invoke<boolean>("cancel_scan"),
    scanSummary: () => invoke<ScanSummary | null>("scan_summary"),
    rows: (parentId, offset, limit) => invoke<RowPage>("rows", { parentId, offset, limit }),

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

/**
 * In-memory Transport for UI development and tests.
 *
 * It reports a plausible ncdu and a two-file tree, and it does not pretend to
 * walk a disk: `startScan` completes immediately with numbers that are visibly a
 * fixture. A mock that looks like a real scan is a mock somebody eventually
 * mistakes for one.
 */
export function createMockTransport(overrides: Partial<Transport> = {}): Transport {
  const summary: ScanSummary = {
    rootId: 0,
    rootPath: "/fixtures/root",
    totalBytes: 4096,
    entries: 3,
    directories: 1,
    backendId: "ncdu",
    backendVersion: "2.8.2",
    readErrors: 0,
    excluded: 0,
    hardlinksDeduplicated: 0,
    hardlinkBytesSaved: 0,
  };

  const rows = [
    {
      id: 1,
      name: "big",
      kind: "file",
      ownBytes: 3072,
      apparentBytes: 3072,
      totalBytes: 3072,
      readError: false,
      hardlink: false,
      excluded: false,
      childCount: 0,
      share: 0.75,
    },
    {
      id: 2,
      name: "small",
      kind: "file",
      ownBytes: 1024,
      apparentBytes: 1024,
      totalBytes: 1024,
      readError: false,
      hardlink: false,
      excluded: false,
      childCount: 0,
      share: 0.25,
    },
  ] satisfies RowPage["rows"];

  let onFinished: ((summary: ScanSummary) => void) | null = null;

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
        },
      ];
    },

    async capabilities() {
      return {
        scan: true,
        delete: true,
        trash: false,
        dryRun: false,
        cleanupCategories: false,
        uninstallApps: false,
        systemStatus: false,
      };
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

    async rows(parentId, offset, limit) {
      return {
        parentId: parentId ?? 0,
        path: summary.rootPath,
        offset,
        total: rows.length,
        rows: rows.slice(offset, offset + limit),
      };
    },

    onScanProgress() {
      return () => {};
    },

    onScanFinished(handler) {
      onFinished = handler;
      return () => {
        onFinished = null;
      };
    },

    onScanFailed() {
      return () => {};
    },
  };

  return { ...base, ...overrides };
}
