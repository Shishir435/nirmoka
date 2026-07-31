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

import type { Backend, Capabilities, Row, ScanProgress } from "./types.js";

export type * from "./types.js";

export type Unsubscribe = () => void;

/**
 * What the frontend needs from a backend, stated without reference to how it
 * gets there.
 */
export interface Transport {
  /** Which disk backends are installed and usable. */
  listBackends(): Promise<Backend[]>;

  /** Capabilities of the active backend, for hiding unsupported controls. */
  capabilities(): Promise<Capabilities>;

  /** Begin a scan. Progress arrives via `onScanProgress`. */
  startScan(rootPath: string): Promise<void>;

  /** Cancel the running scan. Must actually kill the subprocess. */
  cancelScan(): Promise<void>;

  /**
   * Rows for a visible window.
   *
   * The tree lives in Rust. Never request the whole tree — a home directory can
   * be millions of nodes, and rendering that as DOM is the mistake that gets
   * blamed on the GUI framework.
   */
  rows(parentId: number, offset: number, limit: number): Promise<Row[]>;

  onScanProgress(handler: (progress: ScanProgress) => void): Unsubscribe;
}

/**
 * In-memory Transport for UI development and tests.
 *
 * Lets the frontend be built and styled before the Tauri app exists, and keeps
 * component tests from needing a real backend.
 */
export function createMockTransport(overrides: Partial<Transport> = {}): Transport {
  const base: Transport = {
    async listBackends() {
      return [
        {
          id: "ncdu",
          displayName: "ncdu",
          supportedVersions: ">=2.0, <3.0",
          detection: { state: "found", path: "ncdu", version: "2.8.2" },
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

    async startScan() {},
    async cancelScan() {},

    async rows() {
      return [];
    },

    onScanProgress() {
      return () => {};
    },
  };

  return { ...base, ...overrides };
}
