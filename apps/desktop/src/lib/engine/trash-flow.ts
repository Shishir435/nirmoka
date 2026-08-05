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
  /**
   * Which request the reducer is waiting on, or `null` for none.
   *
   * A request needs an identity of its own, and the row's id is not one: node
   * ids are reused. Navigate away and back, or reorder, and asking about the
   * same row again produces a second request wearing the first one's name — at
   * which point a late reply from the abandoned one is indistinguishable from
   * the live one's. Counting requests makes them distinguishable by
   * construction.
   */
  pendingRequestId: number | null;
  /** The row that confirmation is about, so the right one can be marked. */
  pendingNodeId: number | null;
  preparing: boolean;
  running: boolean;
  /**
   * Which move is underway, or `null` for none. Identified for the same reason
   * a preparation is, and the exposure is larger: on macOS the move asks the
   * Finder, which can sit on a permission prompt for as long as the user
   * ignores it. A rescan in that window resets this state, and an unidentified
   * reply landing afterwards would mark whichever row is pending *now*.
   */
  runningRequestId: number | null;
  /** Node ids already in the Trash, for as long as this scan lasts. */
  trashedIds: number[];
  /** The last completed move, for the line that says what happened. */
  last: TrashOperation | null;
  error: string | null;
}

export type TrashEvent =
  /**
   * `requestId` comes from the caller, which is the only place that can hand
   * the same number to a request and to its two possible answers.
   */
  | { type: "prepareStarted"; requestId: number; nodeId: number }
  /**
   * A preparation is a round trip, and the answer can arrive after the user
   * has moved on — see the reducer.
   */
  | { type: "prepared"; requestId: number; preparation: TrashPreparation }
  /** Identified for the same reason `prepared` is. */
  | { type: "prepareFailed"; requestId: number; message: string }
  | { type: "dismissed" }
  /** Its own attempt, with its own number: the move outlives the dialog. */
  | { type: "runStarted"; requestId: number }
  | { type: "trashed"; requestId: number; operation: TrashOperation }
  | { type: "runFailed"; requestId: number; message: string }
  /**
   * Moved to another directory, or reordered. The error is stale, any request
   * in flight is abandoned, and the removals are neither.
   */
  | { type: "moved" }
  /** A rescan renumbers the tree, so ids from the old one name other things. */
  | { type: "rescanned" };

export const INITIAL_TRASH: TrashState = {
  preparation: null,
  pendingRequestId: null,
  pendingNodeId: null,
  preparing: false,
  running: false,
  runningRequestId: null,
  trashedIds: [],
  last: null,
  error: null,
};

export function reduceTrash(state: TrashState, event: TrashEvent): TrashState {
  switch (event.type) {
    case "prepareStarted":
      return {
        ...state,
        preparing: true,
        pendingRequestId: event.requestId,
        pendingNodeId: event.nodeId,
        error: null,
        last: null,
      };

    // Both answers are matched to the request that asked, and only the request
    // still being waited on is answered.
    //
    // Rust replies asynchronously, so a reply can land after the user has
    // changed directory, reordered, or rescanned — and the token it carries
    // would still have executed, because navigating within a scan does not
    // change the scan id `confirm_trash` checks. Ungated, that pops a
    // confirmation for a row nobody can see.
    //
    // Matching on the row is not enough, which is what two rounds of review
    // established: node ids are reused, so a second request about the same row
    // wears the first one's name. `requestId` is unique per attempt.
    case "prepared":
      return answering(state, event.requestId)
        ? { ...state, preparing: false, preparation: event.preparation }
        : state;

    case "prepareFailed":
      // Rust refused before anything moved — a stale scan, a path that is gone,
      // a location the validator protects. There is nothing to confirm.
      //
      // Gated exactly like `prepared`. An abandoned request rejecting while a
      // replacement is in flight would otherwise clear the replacement's
      // pending state, and the replacement's own answer would then be dropped
      // by the guard above — no dialog, and someone else's error under it.
      return answering(state, event.requestId)
        ? {
            ...state,
            preparing: false,
            pendingRequestId: null,
            preparation: null,
            pendingNodeId: null,
            error: event.message,
          }
        : state;

    case "dismissed":
      return { ...state, preparation: null, pendingRequestId: null, pendingNodeId: null };

    case "runStarted":
      // The token is spent the moment it is sent. Keeping the dialog's copy
      // would leave a second click able to submit a confirmation Rust has
      // already consumed.
      return {
        ...state,
        preparation: null,
        running: true,
        runningRequestId: event.requestId,
        error: null,
      };

    // Gated like the preparation replies, and for a sharper reason: this one
    // marks a row as being in the Trash. An unidentified reply landing after a
    // rescan would put that label on whichever row happens to be pending now —
    // the window claiming something is in the Trash that is not.
    case "trashed":
      if (!completing(state, event.requestId)) return state;
      return {
        ...state,
        running: false,
        runningRequestId: null,
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
      //
      // Gated too: a stale failure clearing `running` would re-enable the
      // buttons while a real move is still in flight.
      return completing(state, event.requestId)
        ? {
            ...state,
            running: false,
            runningRequestId: null,
            pendingNodeId: null,
            error: event.message,
          }
        : state;

    // Everything about the old location goes, including a confirmation the
    // user has effectively walked away from. What survives is what is already
    // in the Trash — those rows are still in the Trash from anywhere.
    //
    // A move already running is not cancelled. It cannot be: the item is
    // between here and the Trash, and its result still belongs in the journal.
    case "moved":
      return state.running
        ? { ...state, preparation: null, preparing: false, pendingRequestId: null, error: null }
        : {
            ...state,
            preparation: null,
            pendingRequestId: null,
            pendingNodeId: null,
            preparing: false,
            error: null,
            last: null,
          };

    case "rescanned":
      return INITIAL_TRASH;
  }
}

/**
 * Whether this answer belongs to the request still being waited on.
 *
 * `preparing` alone is not the question: it is true again the moment a
 * replacement starts. Neither is the row, because rows are asked about more
 * than once.
 */
function answering(state: TrashState, requestId: number) {
  return state.preparing && state.pendingRequestId === requestId;
}

/** The same question for the move itself. */
function completing(state: TrashState, requestId: number) {
  return state.running && state.runningRequestId === requestId;
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
