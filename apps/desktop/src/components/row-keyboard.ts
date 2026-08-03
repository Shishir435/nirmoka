/**
 * What a key means in the directory list.
 *
 * The mapping is a pure function of the key and where the selection is, so the
 * awkward cases — an empty directory, the first row, the last row, a page jump
 * past the end — are covered by tests rather than by trying it in the window.
 * The tree view turns an intent into an action; it decides nothing about which
 * intent a key produces.
 */

/** Rows moved by Page Up and Page Down. One screen of the list. */
export const PAGE = 12;

/** Stable element id for a row, so `aria-activedescendant` can name one. */
export const rowElementId = (index: number) => `directory-row-${index}`;

/**
 * The id `aria-activedescendant` may point at.
 *
 * It must name an element that is actually in the DOM. A virtualized list only
 * renders a window, so a selection outside it — for the frame between a jump
 * and the scroll that follows — has no element to name, and pointing at a
 * missing id leaves a screen reader with no active option at all. `undefined`
 * for that frame is recoverable; a dangling reference is not.
 */
export function activeDescendantId(
  selected: number | null,
  rendered: readonly number[],
): string | undefined {
  if (selected === null) return undefined;
  return rendered.includes(selected) ? rowElementId(selected) : undefined;
}

export type RowIntent =
  /** Move the selection to `index`. */
  | { kind: "select"; index: number }
  /** Open the selected row, if it is a directory. */
  | { kind: "open" }
  /** Leave for the parent directory. */
  | { kind: "up" }
  /** Step through visited directories. */
  | { kind: "back" }
  | { kind: "forward" }
  /** Show the selected row in Quick Look. */
  | { kind: "preview" }
  /** Nothing this component handles: leave the event alone. */
  | null;

export interface RowKeyState {
  /** Currently selected row, or `null` when nothing is selected yet. */
  selected: number | null;
  total: number;
}

/**
 * Resolve one keydown.
 *
 * Returning `null` matters as much as the rest: an unhandled key must keep its
 * default behaviour, so the caller only calls `preventDefault` when an intent
 * comes back.
 */
export function rowIntent(key: string, state: RowKeyState): RowIntent {
  const { total } = state;

  // Every key below acts on a row, and there are none. Up and back still work:
  // an empty directory is a place you need to leave.
  if (total <= 0) {
    switch (key) {
      case "ArrowLeft":
      case "Backspace":
        return { kind: "up" };
      case "BrowserBack":
        return { kind: "back" };
      case "BrowserForward":
        return { kind: "forward" };
      default:
        return null;
    }
  }

  const last = total - 1;
  // With nothing selected, the first Down lands on the first row rather than the
  // second, and the first Up lands on the last.
  const from = state.selected;
  const clamp = (index: number) => Math.min(last, Math.max(0, index));
  const select = (index: number): RowIntent => ({ kind: "select", index: clamp(index) });

  switch (key) {
    case "ArrowDown":
      return from === null ? select(0) : select(from + 1);
    case "ArrowUp":
      return from === null ? select(last) : select(from - 1);
    case "PageDown":
      return from === null ? select(0) : select(from + PAGE);
    case "PageUp":
      return from === null ? select(last) : select(from - PAGE);
    case "Home":
      return select(0);
    case "End":
      return select(last);
    case "Enter":
    case "ArrowRight":
      return from === null ? null : { kind: "open" };
    case " ":
      return from === null ? null : { kind: "preview" };
    case "ArrowLeft":
    case "Backspace":
      return { kind: "up" };
    case "BrowserBack":
      return { kind: "back" };
    case "BrowserForward":
      return { kind: "forward" };
    default:
      return null;
  }
}

/**
 * The label a screen reader reads for one row.
 *
 * Built here rather than in JSX because the interesting part is what it says
 * when the number is not the whole story: an unreadable directory's size is a
 * lower bound, and a deduplicated hardlink is counted somewhere else.
 */
export function rowLabel(row: {
  name: string;
  kind: string;
  size: string;
  childCount: number;
  readError: boolean;
  excluded: boolean;
  hardlink: boolean;
}) {
  const parts = [row.kind === "directory" ? `Folder ${row.name}` : `File ${row.name}`, row.size];

  if (row.childCount > 0) {
    parts.push(`${row.childCount} ${row.childCount === 1 ? "entry" : "entries"}`);
  }
  if (row.readError) parts.push("could not be read, size is a lower bound");
  if (row.excluded) parts.push("excluded from the scan");
  if (row.hardlink) parts.push("hardlink, counted once elsewhere");

  return parts.join(", ");
}
