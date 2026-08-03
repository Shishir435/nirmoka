/**
 * What the window believes about the current scan.
 *
 * A reducer rather than a pile of `setScan` calls, because the transitions have
 * rules that are easy to get wrong and hard to notice: progress for a scan that
 * has already finished must not reopen it, a cancellation is not a failure, and
 * a scan that a rescan replaced must not have its late events applied.
 *
 * Only type imports, so the module runs under `node --test` with no DOM.
 */

import type { ScanFailure, ScanProgress, ScanSummary } from "@nirmoka/transport";

export type ScanState =
  | { status: "idle" }
  | { status: "scanning"; root: string; progress: ScanProgress }
  | { status: "done"; summary: ScanSummary }
  | { status: "cancelled" }
  | { status: "failed"; message: string };

export type ScanEvent =
  /** The user asked for a scan of `root`, before Rust has confirmed anything. */
  | { type: "requested"; root: string }
  /** Rust answered with the canonical root, which differs whenever the request
   *  was relative or went through a symlink. */
  | { type: "rooted"; root: string }
  | { type: "progress"; progress: ScanProgress }
  | { type: "finished"; summary: ScanSummary }
  | { type: "failed"; failure: ScanFailure }
  /** A scan summary found at startup: a scan this window did not watch. */
  | { type: "restored"; summary: ScanSummary };

export const INITIAL_SCAN: ScanState = { status: "idle" };

export function reduceScan(state: ScanState, event: ScanEvent): ScanState {
  switch (event.type) {
    case "requested":
      // A rescan starts from scratch even while another is in flight: the token
      // in Rust is what actually stops the old one, and showing its progress
      // under the new root would attribute one scan's numbers to another.
      return {
        status: "scanning",
        root: event.root,
        progress: { scanned: 0, currentPath: event.root },
      };

    case "rooted":
      // Late arrival for a scan that has already ended. Applying it would
      // reopen a finished scan.
      if (state.status !== "scanning") return state;
      return {
        ...state,
        root: event.root,
        progress: { ...state.progress, currentPath: event.root },
      };

    case "progress":
      if (state.status !== "scanning") return state;
      return { ...state, progress: event.progress };

    case "finished":
    case "restored":
      return { status: "done", summary: event.summary };

    case "failed":
      // Cancellation is a user's decision, not a fault. Reporting it as an
      // error message would put a red line under a button the user just pressed.
      return event.failure.cancelled
        ? { status: "cancelled" }
        : { status: "failed", message: event.failure.message };
  }
}

/**
 * Whether the Scan button can do anything.
 *
 * Both conditions are real failures seen in this app: with no usable scanner
 * the command has nothing to run, and starting before the event listeners are
 * registered loses the terminal event, leaving the window on "scanning" forever.
 */
export function canStartScan(options: {
  scanner: string | null | undefined;
  listenersReady: boolean;
  state: ScanState;
}) {
  return options.scanner != null && options.listenersReady && options.state.status !== "scanning";
}
