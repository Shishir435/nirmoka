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
  | { type: "prepared"; preparation: TrashPreparation }
  | { type: "prepareFailed"; message: string }
  | { type: "dismissed" }
  | { type: "runStarted" }
  | { type: "trashed"; operation: TrashOperation }
  | { type: "runFailed"; message: string }
  /** Moved to another directory: the error is stale, the removals are not. */
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

    case "prepared":
      return { ...state, preparing: false, preparation: event.preparation };

    case "prepareFailed":
      // Rust refused before anything moved — a stale scan, a path that is gone,
      // a location the validator protects. There is nothing to confirm.
      return {
        ...state,
        preparing: false,
        preparation: null,
        pendingNodeId: null,
        error: event.message,
      };

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

    case "moved":
      return { ...state, error: null, last: null };

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
