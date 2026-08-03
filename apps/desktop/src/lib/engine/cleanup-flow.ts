/**
 * The cleanup review-and-run flow, as data.
 *
 * The clean page has one sequence with several ways to end: review, confirm,
 * run, and then finished, partial, stopped, or failed. Those endings are where
 * honesty lives — a stopped run still removed files — so the transitions are a
 * reducer that tests can drive end to end, rather than a set of booleans that
 * can only be checked by clicking.
 *
 * One rule the reducer enforces on the window's behalf: the reviewed preview is
 * dropped the moment a run starts. Rust has already forgotten it (ADR 0020), so
 * a UI still showing those paths would be describing the past as the present.
 *
 * Only type imports, so the module runs under `node --test` with no DOM.
 */

import type { CleanupOperation, CleanupPreparation, CleanupPreview } from "@nirmoka/transport";

export interface CleanupState {
  /** The reviewed preview, or `null` once it is spent or never taken. */
  preview: CleanupPreview | null;
  /** A live confirmation, which the dialog is open for. */
  preparation: CleanupPreparation | null;
  previewing: boolean;
  running: boolean;
  /** The user asked to stop; Mole is being killed. */
  stopping: boolean;
  /** The result of the last run, whatever kind of ending it had. */
  result: CleanupOperation | null;
  previewError: string | null;
  runError: string | null;
}

export type CleanupEvent =
  | { type: "previewStarted" }
  | { type: "previewArrived"; preview: CleanupPreview }
  | { type: "previewFailed"; message: string }
  | { type: "reviewed"; preparation: CleanupPreparation }
  | { type: "reviewFailed"; message: string }
  | { type: "reviewDismissed" }
  | { type: "runStarted" }
  | { type: "stopRequested" }
  | { type: "runFinished"; operation: CleanupOperation }
  | { type: "runFailed"; message: string };

export const INITIAL_CLEANUP: CleanupState = {
  preview: null,
  preparation: null,
  previewing: false,
  running: false,
  stopping: false,
  result: null,
  previewError: null,
  runError: null,
};

export function reduceCleanup(state: CleanupState, event: CleanupEvent): CleanupState {
  switch (event.type) {
    case "previewStarted":
      return { ...state, previewing: true, previewError: null, runError: null };

    case "previewArrived":
      return { ...state, previewing: false, preview: event.preview, previewError: null };

    case "previewFailed":
      return { ...state, previewing: false, previewError: event.message };

    case "reviewed":
      return { ...state, preparation: event.preparation, result: null, runError: null };

    case "reviewFailed":
      // A confirmation that could not be prepared is a stale or empty preview.
      // Keeping it on screen would offer a review of something Rust will refuse.
      return { ...state, preparation: null, preview: null, runError: event.message };

    case "reviewDismissed":
      return { ...state, preparation: null };

    case "runStarted":
      return {
        ...state,
        preparation: null,
        // Spent the moment Mole starts: it re-discovers candidates, so these
        // paths describe a discovery that has been superseded.
        preview: null,
        running: true,
        stopping: false,
        result: null,
        runError: null,
      };

    case "stopRequested":
      return state.running ? { ...state, stopping: true } : state;

    case "runFinished":
      return { ...state, running: false, stopping: false, result: event.operation };

    case "runFailed":
      // Only a run that never started reaches here; the adapter reports an
      // interrupted run as a result. See ADR 0020.
      return { ...state, running: false, stopping: false, runError: event.message };
  }
}

/** Whether there is a non-empty review worth confirming. */
export function canReview(state: CleanupState) {
  return (
    !state.previewing && !state.running && state.preview !== null && state.preview.totalItems > 0
  );
}

/** What the result card should call the ending. */
export function outcomeLabel(completion: CleanupOperation["completion"]) {
  switch (completion) {
    case "finished":
      return "Finished";
    case "partial":
      return "Partial";
    case "cancelled":
      return "Stopped";
    case "failed":
      return "Failed";
  }
}

/**
 * Only a clean finish is good news. A stopped or failed run removed an unknown
 * amount before it ended, which is a warning, not a success.
 */
export function outcomeTone(completion: CleanupOperation["completion"]) {
  return completion === "finished" ? "success" : "warning";
}
