/**
 * Selecting a row, confirming it, and moving it to the Trash — as data.
 *
 * The sequence is short and the honesty is in what happens afterwards. A
 * trashed row does not vanish: the scan still knows about it, every total above
 * it was measured before the move, and pretending otherwise would mean redrawing
 * numbers nobody measured. So the row stays, marked, until a rescan.
 *
 * There is also no undo step here. Recovery is the platform's Put Back, in the
 * Finder — see ADR 0025 — and a button in this window would have to guess a path
 * inside the Trash that the system may have renamed.
 *
 * Only type imports, so the module runs under `node --test` with no DOM.
 */

import type { Row, TrashOperation, TrashPreparation } from "@nirmoka/transport";

export interface TrashState {
  /** A live confirmation, which the dialog is open for. */
  preparation: TrashPreparation | null;
  /** The row that confirmation is about, so the right one can be marked. */
  pendingNodeId: number | null;
  preparing: boolean;
  running: boolean;
  /** Node ids already in the Trash, for as long as this scan lasts. */
  trashedIds: number[];
  /** The last completed move, for the line that says what happened. */
  last: TrashOperation | null;
  error: string | null;
}

export type TrashEvent =
  | { type: "prepareStarted"; nodeId: number }
  /**
   * Carries the row it was asked for. A preparation is a network round trip,
   * and the answer can arrive after the user has moved on — see the reducer.
   */
  | { type: "prepared"; nodeId: number; preparation: TrashPreparation }
  /** Carries the row for the same reason `prepared` does. */
  | { type: "prepareFailed"; nodeId: number; message: string }
  | { type: "dismissed" }
  | { type: "runStarted" }
  | { type: "trashed"; operation: TrashOperation }
  | { type: "runFailed"; message: string }
  /**
   * Moved to another directory, or reordered. The error is stale, any request
   * in flight is abandoned, and the removals are neither.
   */
  | { type: "moved" }
  /** A rescan renumbers the tree, so ids from the old one name other things. */
  | { type: "rescanned" };

export const INITIAL_TRASH: TrashState = {
  preparation: null,
  pendingNodeId: null,
  preparing: false,
  running: false,
  trashedIds: [],
  last: null,
  error: null,
};

export function reduceTrash(state: TrashState, event: TrashEvent): TrashState {
  switch (event.type) {
    case "prepareStarted":
      return { ...state, preparing: true, pendingNodeId: event.nodeId, error: null, last: null };

    // A preparation that nobody is waiting for any more is dropped rather than
    // shown. Rust answers asynchronously, so the reply can land after the user
    // has changed directory or reordered the list — and the token it carries
    // is still executable, because navigating within a scan does not change
    // the scan id that `confirm_trash` checks. Left ungated, leaving a
    // directory mid-request pops a confirmation for a row that is no longer
    // there. The row is matched too, so an answer to a superseded request
    // cannot install itself over a newer one.
    case "prepared":
      return state.preparing && state.pendingNodeId === event.nodeId
        ? { ...state, preparing: false, preparation: event.preparation }
        : state;

    case "prepareFailed":
      // Rust refused before anything moved — a stale scan, a path that is gone,
      // a location the validator protects. There is nothing to confirm.
      //
      // Gated exactly like `prepared`, and on the row rather than on
      // `preparing` alone. `preparing` is true again as soon as a replacement
      // request starts, so an abandoned request rejecting at that moment would
      // clear the replacement's pending row — and the replacement's own answer
      // would then be dropped by the guard above, leaving no dialog and a stale
      // error. The row is what tells the two apart.
      return state.preparing && state.pendingNodeId === event.nodeId
        ? {
            ...state,
            preparing: false,
            preparation: null,
            pendingNodeId: null,
            error: event.message,
          }
        : state;

    case "dismissed":
      return { ...state, preparation: null, pendingNodeId: null };

    case "runStarted":
      // The token is spent the moment it is sent. Keeping the dialog's copy
      // would leave a second click able to submit a confirmation Rust has
      // already consumed.
      return { ...state, preparation: null, running: true, error: null };

    case "trashed":
      return {
        ...state,
        running: false,
        last: event.operation,
        pendingNodeId: null,
        trashedIds:
          state.pendingNodeId === null || state.trashedIds.includes(state.pendingNodeId)
            ? state.trashedIds
            : [...state.trashedIds, state.pendingNodeId],
      };

    case "runFailed":
      // Nothing moved, so the row is not marked. The one failure worth naming
      // separately — the desktop refusing an Apple event — arrives here with
      // the setting that grants it, from Rust.
      return { ...state, running: false, pendingNodeId: null, error: event.message };

    // Everything about the old location goes, including a confirmation the
    // user has effectively walked away from. What survives is what is already
    // in the Trash — those rows are still in the Trash from anywhere.
    //
    // A move already running is not cancelled. It cannot be: the item is
    // between here and the Trash, and its result still belongs in the journal.
    case "moved":
      return state.running
        ? { ...state, preparation: null, preparing: false, error: null }
        : {
            ...state,
            preparation: null,
            pendingNodeId: null,
            preparing: false,
            error: null,
            last: null,
          };

    case "rescanned":
      return INITIAL_TRASH;
  }
}

/** Whether this row can be moved to the Trash right now. */
export function canTrash(state: TrashState, row: Row | undefined): row is Row {
  return (
    row !== undefined &&
    !state.preparing &&
    !state.running &&
    state.preparation === null &&
    !state.trashedIds.includes(row.id)
  );
}

/** Whether this row is one of the ones already moved. */
export function isTrashed(state: TrashState, row: Row) {
  return state.trashedIds.includes(row.id);
}

/**
 * What to say after a move.
 *
 * The journal write failing does not mean the move failed — the item is in the
 * Trash either way — so this reports both facts rather than picking one.
 */
export function outcomeMessage(operation: TrashOperation) {
  const moved = `${operation.targetPath} is in the Trash. Recover it there with Put Back.`;
  return operation.logError === null
    ? moved
    : `${moved} It could not be added to the operation log: ${operation.logError}`;
}
