/**
 * One directory, paged in as it is scrolled.
 *
 * # Why this is not just `useState(rows)`
 *
 * Invariant 5: the tree stays in Rust and this window holds only what it is
 * about to paint. A directory can have a hundred thousand children, so the hook
 * holds a sparse array — `total` slots, filled a chunk at a time — and asks for
 * a chunk only when the virtualizer scrolls a row of it into view.
 *
 * Sorting is a request parameter for the same reason. Reordering the rows this
 * hook happens to be holding would sort the visible slice and leave the rest of
 * the directory where it was.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type { Crumb, Row, Sort, Transport } from "@nirmoka/transport";

/**
 * Rows per request. Big enough that a scroll of one screen is usually already
 * loaded, small enough that a directory of a hundred thousand entries never
 * arrives in one message. The Rust side caps anything larger regardless.
 */
export const CHUNK = 100;

/** What the directory is, separate from what is in it. */
export interface DirectoryHeader {
  parentId: number;
  name: string;
  path: string;
  ancestors: Crumb[];
  /** The directory itself could not be read: empty means "not allowed to look". */
  readError: boolean;
  total: number;
  sort: Sort;
}

export type Directory =
  | { status: "loading" }
  | { status: "failed"; message: string }
  | {
      status: "ready";
      header: DirectoryHeader;
      /** `undefined` for a row whose chunk has not arrived: render a placeholder. */
      rowAt: (index: number) => Row | undefined;
      /** Ask for the chunks covering a visible range. Idempotent. */
      ensure: (start: number, end: number) => void;
    };

/** The ready state before `ensure` is attached, which the hook does on return. */
type Loaded = Extract<Directory, { status: "ready" }>;

interface Request {
  scanId: number;
  parentId: number | null;
  sort: Sort;
}

export function useDirectory(transport: Transport, request: Request | null): Directory {
  const [state, setState] = useState<Omit<Loaded, "ensure"> | Exclude<Directory, Loaded>>({
    status: "loading",
  });

  /** Sparse by design: `undefined` is a row that has not been asked for yet. */
  const rows = useRef<(Row | undefined)[]>([]);
  const inFlight = useRef(new Set<number>());
  /** Bumped when a chunk lands, to repaint rows that were placeholders. */
  const [, setLoaded] = useState(0);

  /**
   * Which directory-and-order `rows.current` currently holds.
   *
   * Every request captures this and is discarded on arrival if it no longer
   * matches. Comparing the page against the header instead would not be enough:
   * a chunk still in flight when the user navigates away comes back describing
   * the directory it was asked for — the comparison passes — and writes into
   * the array that now belongs to a different one.
   */
  const generation = useRef(0);

  const { scanId, parentId, sort } = request ?? {};

  useEffect(() => {
    if (!request) return;

    let live = true;
    const era = (generation.current += 1);
    rows.current = [];
    inFlight.current.clear();
    setState({ status: "loading" });

    transport
      .rows(request.scanId, request.parentId, request.sort, 0, CHUNK)
      .then((page) => {
        if (!live || generation.current !== era) return;

        rows.current = new Array<Row | undefined>(page.total);
        page.rows.forEach((row, index) => {
          rows.current[index] = row;
        });

        setState({
          status: "ready",
          header: {
            parentId: page.parentId,
            name: page.name,
            path: page.path,
            ancestors: page.ancestors,
            readError: page.readError,
            total: page.total,
            // The page's own answer, not what was asked for: while a request is
            // in flight those are different, and the controls should describe
            // what is on screen.
            sort: page.sort,
          },
          rowAt: (index) => rows.current[index],
        });
      })
      .catch((error: unknown) => {
        if (live) setState({ status: "failed", message: String(error) });
      });

    return () => {
      live = false;
    };
    // The request object is rebuilt every render; its three fields are what
    // actually identify a directory.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [transport, scanId, parentId, sort]);

  /**
   * Ask for whatever chunks cover `[start, end]` and are not already here or on
   * their way. Safe to call on every scroll frame.
   */
  const ensure = useCallback(
    (start: number, end: number) => {
      if (!request || state.status !== "ready") return;

      const first = Math.floor(Math.max(0, start) / CHUNK);
      const last = Math.floor(Math.min(end, state.header.total - 1) / CHUNK);

      const era = generation.current;

      for (let chunk = first; chunk <= last; chunk += 1) {
        const offset = chunk * CHUNK;
        if (inFlight.current.has(offset) || rows.current[offset] !== undefined) continue;

        inFlight.current.add(offset);
        transport
          .rows(request.scanId, request.parentId, request.sort, offset, CHUNK)
          .then((page) => {
            // Rows the user has since navigated away from would land in the
            // array that now belongs to a different directory. The page itself
            // cannot say so — it correctly describes what was asked for.
            if (generation.current !== era) return;

            page.rows.forEach((row, index) => {
              rows.current[offset + index] = row;
            });
            setLoaded((n) => n + 1);
          })
          .catch(() => {
            // A chunk that failed stays a placeholder and can be asked for
            // again on the next scroll. Replacing the whole directory with an
            // error because one window did not arrive loses everything already
            // on screen.
            inFlight.current.delete(offset);
          });
      }
    },
    [transport, request, state],
  );

  return state.status === "ready" ? { ...state, ensure } : state;
}
