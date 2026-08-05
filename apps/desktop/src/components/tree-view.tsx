/**
 * The directory browser.
 *
 * Rows are virtualized from the first commit rather than after it got slow. The
 * DOM holds the rows on screen and a little either side; the rest of the
 * directory stays in Rust and arrives a chunk at a time as it is scrolled to.
 * A directory of a hundred thousand entries rendered as a hundred thousand
 * `<li>` elements is the failure that gets blamed on the GUI framework.
 *
 * # Keyboard and screen readers
 *
 * The list is a `listbox` with one selected row, driven by `aria-activedescendant`
 * rather than by focusing each row. Focusing rows would fight the virtualizer:
 * the element holding focus is unmounted as soon as it scrolls out of the
 * overscan window, and focus would land back on the document. One focused
 * container that names its active row survives scrolling.
 *
 * Which key means what lives in `row-keyboard.ts`, so the edge cases — an empty
 * directory, a page jump past the end — are covered by tests.
 */

import { Eye, FolderOpen, Trash2 } from "lucide-react";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";

import type { PlatformFeatures, Row, ScanSummary, Sort, Transport } from "@nirmoka/transport";
import { useVirtualizer } from "@tanstack/react-virtual";

import { activeDescendantId, rowElementId, rowIntent, rowLabel } from "@/components/row-keyboard";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useDirectory, type DirectoryHeader } from "@/hooks/use-directory";
import {
  canTrash,
  INITIAL_TRASH,
  isTrashed,
  outcomeMessage,
  reduceTrash,
} from "@/lib/engine/trash-flow";
import { formatBytes, formatCount, plural } from "@/lib/format";

/** Row height in pixels. The virtualizer needs this before it measures. */
const ROW_HEIGHT = 36;

const SORTS: { value: Sort; label: string }[] = [
  { value: "largestFirst", label: "Largest" },
  { value: "smallestFirst", label: "Smallest" },
  { value: "nameAscending", label: "Name A–Z" },
  { value: "nameDescending", label: "Name Z–A" },
];

function Bar({ share }: { share: number }) {
  return (
    <div className="bg-muted h-1.5 w-20 shrink-0 overflow-hidden rounded-full">
      <div className="bg-brand h-full rounded-full" style={{ width: `${share * 100}%` }} />
    </div>
  );
}

/** Flags that mean "this number is not the whole story". */
function Flags({ row }: { row: Row }) {
  const notes = [
    row.readError && "unreadable",
    row.excluded && "excluded",
    row.hardlink && "hardlink",
  ].filter(Boolean);

  if (notes.length === 0) return null;

  return <span className="text-muted-foreground shrink-0 text-xs">{notes.join(" · ")}</span>;
}

function Breadcrumb({
  header,
  onNavigate,
}: {
  header: DirectoryHeader;
  onNavigate: (id: number | null) => void;
}) {
  return (
    <nav aria-label="Location" className="flex flex-wrap items-center gap-1 text-sm">
      {header.ancestors.map((crumb) => (
        <span key={crumb.id} className="flex items-center gap-1">
          <button
            type="button"
            onClick={() => onNavigate(crumb.id)}
            className="hover:text-foreground text-muted-foreground truncate font-mono underline-offset-4 hover:underline"
          >
            {crumb.name}
          </button>
          <span className="text-muted-foreground">/</span>
        </span>
      ))}
      <span className="truncate font-mono">{header.name}</span>
    </nav>
  );
}

function SortControls({ sort, onSort }: { sort: Sort; onSort: (sort: Sort) => void }) {
  return (
    <div className="flex gap-1">
      {SORTS.map((option) => (
        <Button
          key={option.value}
          variant={option.value === sort ? "secondary" : "ghost"}
          size="sm"
          aria-pressed={option.value === sort}
          onClick={() => onSort(option.value)}
        >
          {option.label}
        </Button>
      ))}
    </div>
  );
}

/**
 * A row whose chunk has not arrived yet.
 *
 * It occupies the height it will occupy, so the scrollbar does not jump when
 * the real row lands — and it is a real option with a real name, because the
 * selection can land on it. A placeholder rendered as an anonymous `div` would
 * leave `aria-activedescendant` pointing at something with no role and no
 * accessible name for as long as the chunk takes to arrive.
 */
function Placeholder({ index, selected }: { index: number; selected: boolean }) {
  return (
    <div
      id={rowElementId(index)}
      role="option"
      aria-selected={selected}
      aria-busy
      aria-label={`Loading entry ${index + 1}`}
      tabIndex={-1}
      className={`flex h-9 w-full items-center px-4 ${selected ? "bg-accent" : ""}`}
    >
      <span className="bg-muted/40 h-3 w-full animate-pulse rounded" />
    </div>
  );
}

export function TreeView({
  transport,
  summary,
  parentId,
  sort,
  onNavigate,
  onBack,
  onForward,
  canGoBack,
  canGoForward,
  onSort,
}: {
  transport: Transport;
  summary: ScanSummary;
  parentId: number | null;
  sort: Sort;
  onNavigate: (id: number | null) => void;
  onBack: () => void;
  onForward: () => void;
  canGoBack: boolean;
  canGoForward: boolean;
  onSort: (sort: Sort) => void;
}) {
  const directory = useDirectory(transport, { scanId: summary.scanId, parentId, sort });
  const scroller = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [features, setFeatures] = useState<PlatformFeatures | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [trash, dispatchTrash] = useReducer(reduceTrash, INITIAL_TRASH);

  const total = directory.status === "ready" ? directory.header.total : 0;

  const virtualizer = useVirtualizer({
    count: total,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  const items = virtualizer.getVirtualItems();
  const renderedIndexes = items.map((item) => item.index);
  const ensure = directory.status === "ready" ? directory.ensure : null;
  const first = items[0]?.index ?? 0;
  const last = items[items.length - 1]?.index ?? 0;

  // Asking from an effect rather than during render: fetching is a side effect,
  // and the visible range is only known once the virtualizer has measured.
  useEffect(() => {
    ensure?.(first, last);
  }, [ensure, first, last]);

  // A new directory or a new order starts at the top with nothing selected. The
  // scroll container outlives both, so without this, opening a directory from
  // row 400 lands 400 rows into the one it opened.
  useEffect(() => {
    if (scroller.current) scroller.current.scrollTop = 0;
    setSelected(null);
    setActionError(null);
    dispatchTrash({ type: "moved" });
  }, [parentId, sort]);

  // A rescan renumbers the tree from zero, so an id remembered as "already in
  // the Trash" would mark whichever row now happens to hold that index.
  useEffect(() => {
    dispatchTrash({ type: "rescanned" });
  }, [summary.scanId]);

  // What this desktop can do with a path, asked once. A button labelled
  // "Reveal in Finder" on another platform would be a macOS habit leaking out.
  useEffect(() => {
    let live = true;
    transport.platformFeatures().then(
      (value) => live && setFeatures(value),
      () => live && setFeatures(null),
    );
    return () => {
      live = false;
    };
  }, [transport]);

  // Held as a local so the dialog's callbacks narrow it: a reducer field cannot
  // be narrowed across a closure, and the alternative is a `!` on a token that
  // decides whether a file moves.
  const pendingTrash = trash.preparation;
  const rowAt = directory.status === "ready" ? directory.rowAt : null;
  const selectedRow = selected === null ? undefined : rowAt?.(selected);
  const parent = directory.status === "ready" ? directory.header.ancestors.at(-1) : undefined;

  // Leaving for the parent is a navigation like any other, so it joins the
  // history. At the scan root there is no parent and nothing to do.
  const goUp = useCallback(() => {
    if (parent !== undefined) onNavigate(parent.id);
    else if (parentId !== null) onNavigate(null);
  }, [onNavigate, parent, parentId]);

  const reveal = useCallback(
    (row: Row) => {
      setActionError(null);
      transport.revealInFileManager(summary.scanId, row.id).catch((reason: unknown) => {
        setActionError(String(reason));
      });
    },
    [summary.scanId, transport],
  );

  const preview = useCallback(
    (row: Row) => {
      setActionError(null);
      transport.quickLook(summary.scanId, row.id).catch((reason: unknown) => {
        setActionError(String(reason));
      });
    },
    [summary.scanId, transport],
  );

  const askToTrash = useCallback(
    (row: Row) => {
      setActionError(null);
      dispatchTrash({ type: "prepareStarted", nodeId: row.id });
      transport.prepareTrash(summary.scanId, row.id).then(
        (preparation) => dispatchTrash({ type: "prepared", preparation }),
        (reason: unknown) => dispatchTrash({ type: "prepareFailed", message: String(reason) }),
      );
    },
    [summary.scanId, transport],
  );

  const doTrash = useCallback(
    (confirmationToken: number) => {
      dispatchTrash({ type: "runStarted" });
      transport.confirmTrash(confirmationToken).then(
        (operation) => dispatchTrash({ type: "trashed", operation }),
        (reason: unknown) => dispatchTrash({ type: "runFailed", message: String(reason) }),
      );
    },
    [transport],
  );

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    // ⌘⌫ is the desktop's own gesture for this, and it opens the same
    // confirmation the button does rather than acting on the keypress. It is
    // handled before the guard below because it is precisely a platform
    // shortcut, not a shortcut of ours that a modifier happens to reach.
    if ((event.metaKey || event.ctrlKey) && event.key === "Backspace") {
      if (canTrash(trash, selectedRow)) {
        event.preventDefault();
        askToTrash(selectedRow);
      }
      return;
    }

    // Any other modified key belongs to the platform: ⌘↓ and friends are not
    // ours.
    if (event.metaKey || event.ctrlKey || event.altKey) return;

    const intent = rowIntent(event.key, { selected, total });
    if (!intent) return;
    event.preventDefault();

    switch (intent.kind) {
      case "select":
        setSelected(intent.index);
        // Keep the selection on screen: it is the thing a screen reader is
        // reading and the thing the action buttons act on.
        virtualizer.scrollToIndex(intent.index, { align: "auto" });
        return;
      case "open":
        if (selectedRow?.kind === "directory") onNavigate(selectedRow.id);
        return;
      case "preview":
        if (selectedRow && features?.quickLook) preview(selectedRow);
        return;
      case "up":
        goUp();
        return;
      case "back":
        onBack();
        return;
      case "forward":
        onForward();
        return;
    }
  };

  if (directory.status === "loading") {
    return <p className="text-muted-foreground text-sm">Reading the directory…</p>;
  }

  if (directory.status === "failed") {
    return (
      <div className="space-y-2">
        <p className="text-destructive text-sm">{directory.message}</p>
        {parentId !== null && (
          <Button variant="outline" size="sm" onClick={() => onNavigate(null)}>
            Back to {summary.rootPath}
          </Button>
        )}
      </div>
    );
  }

  const { header } = directory;
  const up = header.ancestors.at(-1);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Breadcrumb header={header} onNavigate={onNavigate} />
        <SortControls sort={header.sort} onSort={onSort} />
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Button variant="outline" size="sm" onClick={onBack} disabled={!canGoBack}>
          Back
        </Button>
        <Button variant="outline" size="sm" onClick={onForward} disabled={!canGoForward}>
          Forward
        </Button>
        {up !== undefined && (
          <Button variant="outline" size="sm" onClick={goUp}>
            Up to {up.name}
          </Button>
        )}
        {features && (
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => selectedRow && reveal(selectedRow)}
              disabled={!selectedRow}
            >
              <FolderOpen />
              {features.revealLabel}
            </Button>
            {features.quickLook && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => selectedRow && preview(selectedRow)}
                disabled={!selectedRow}
              >
                <Eye />
                Quick Look
              </Button>
            )}
            <Button
              variant="outline"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={() => canTrash(trash, selectedRow) && askToTrash(selectedRow)}
              disabled={!canTrash(trash, selectedRow)}
            >
              <Trash2 />
              {trash.running ? "Moving…" : features.trashLabel}
            </Button>
          </>
        )}
      </div>

      <div className="text-muted-foreground flex items-center gap-3 text-xs">
        <span className="truncate font-mono">{header.path}</span>
        <span>{plural(header.total, "entry", "entries")}</span>
      </div>

      {actionError && <p className="text-destructive text-xs">{actionError}</p>}
      {trash.error && <p className="text-destructive text-xs">{trash.error}</p>}
      {trash.last && <p className="text-muted-foreground text-xs">{outcomeMessage(trash.last)}</p>}
      {trash.trashedIds.length > 0 && (
        <p className="text-muted-foreground text-xs">
          Sizes on this page were measured before{" "}
          {plural(trash.trashedIds.length, "item was", "items were")} moved. Rescan for current
          totals.
        </p>
      )}

      {header.total === 0 ? (
        <p className="text-muted-foreground rounded-lg border border-dashed px-4 py-8 text-center text-sm">
          {header.readError
            ? "This directory could not be read. Its size is a lower bound, and what is inside it is unknown — usually a permissions problem."
            : "Nothing here."}
        </p>
      ) : (
        <div
          ref={scroller}
          onKeyDown={onKeyDown}
          className="h-104 overflow-auto rounded-lg border focus-visible:ring-3 focus-visible:ring-ring/20 focus-visible:outline-none"
          role="listbox"
          aria-label="Directory entries"
          aria-activedescendant={activeDescendantId(selected, renderedIndexes)}
          // The list takes focus itself rather than each row: a focused row is
          // unmounted the moment it scrolls out of the virtualizer's window.
          // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- a listbox is focusable by design
          tabIndex={0}
        >
          <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
            {items.map((item) => {
              const row = directory.rowAt(item.index);

              return (
                <div
                  key={item.key}
                  // A positioning wrapper, so it must not sit between the
                  // listbox and its options as far as assistive tech is
                  // concerned.
                  role="presentation"
                  className="absolute top-0 left-0 w-full"
                  style={{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }}
                >
                  {row ? (
                    <RowLine
                      row={row}
                      index={item.index}
                      selected={item.index === selected}
                      trashed={isTrashed(trash, row)}
                      onSelect={() => setSelected(item.index)}
                      onOpen={() => onNavigate(row.id)}
                    />
                  ) : (
                    <Placeholder index={item.index} selected={item.index === selected} />
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      <Dialog
        open={pendingTrash !== null}
        onOpenChange={(open) => !open && dispatchTrash({ type: "dismissed" })}
      >
        <DialogContent>
          {pendingTrash && (
            <>
              <DialogHeader>
                <DialogTitle>
                  {features?.trashLabel ?? "Move to Trash"}
                  {pendingTrash.isDirectory ? " this folder?" : " this item?"}
                </DialogTitle>
                <DialogDescription>{pendingTrash.warning}</DialogDescription>
              </DialogHeader>
              <dl className="space-y-2 text-sm">
                <div className="flex justify-between gap-4">
                  <dt className="text-muted-foreground shrink-0">Path</dt>
                  {/* The resolved path from Rust, not the row's name. What the
                      confirmation names has to be what was checked. */}
                  <dd className="truncate text-right font-mono text-xs">
                    {pendingTrash.targetPath}
                  </dd>
                </div>
                <div className="flex justify-between gap-4">
                  <dt className="text-muted-foreground shrink-0">Size</dt>
                  <dd className="text-right font-mono">{formatBytes(pendingTrash.totalBytes)}</dd>
                </div>
              </dl>
              <div className="mt-6 flex justify-end gap-2">
                <Button variant="outline" onClick={() => dispatchTrash({ type: "dismissed" })}>
                  Cancel
                </Button>
                <Button onClick={() => doTrash(pendingTrash.confirmationToken)}>
                  {features?.trashLabel ?? "Move to Trash"}
                </Button>
              </div>
            </>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function RowLine({
  row,
  index,
  selected,
  trashed,
  onSelect,
  onOpen,
}: {
  row: Row;
  index: number;
  selected: boolean;
  /** Already moved to the Trash by this session, and still listed. */
  trashed: boolean;
  onSelect: () => void;
  onOpen: () => void;
}) {
  // Every directory opens, including the ones with nothing in them. Gating on
  // `childCount > 0` would make the two states worth distinguishing — empty,
  // and unreadable — the two the user cannot reach: an unreadable directory has
  // no children precisely because it could not be read, and refusing to open it
  // leaves the reason unsaid.
  const openable = row.kind === "directory";
  const size = formatBytes(row.totalBytes);

  const content = (
    <>
      <span
        className={`flex-1 truncate text-left font-mono text-sm ${
          trashed ? "text-muted-foreground line-through" : ""
        }`}
      >
        {row.name}
        {row.kind === "directory" ? "/" : ""}
      </span>
      {/* The row stays. Removing it would renumber the list under the
          virtualizer, and the size beside it was measured before the move —
          striking it through says both things at once. */}
      {trashed && <span className="text-muted-foreground shrink-0 text-xs">in the Trash</span>}
      <Flags row={row} />
      {row.childCount > 0 && (
        <span className="text-muted-foreground shrink-0 text-xs">
          {formatCount(row.childCount)}
        </span>
      )}
      <Bar share={row.share} />
      <span className="w-20 shrink-0 text-right font-mono text-sm tabular-nums">{size}</span>
    </>
  );

  const shared = `border-border/60 flex h-9 w-full items-center gap-3 border-b px-4 ${
    selected ? "bg-accent" : ""
  }`;

  // A row is an option in the list rather than a focusable control: the
  // container holds focus and names the active row, which is what survives the
  // virtualizer unmounting rows as they scroll away. Keyboard handling lives on
  // that container for the same reason, which is why the click handlers here
  // have no keyboard twin — the listbox already has one.
  const label = `${rowLabel({
    name: row.name,
    kind: row.kind,
    size,
    childCount: row.childCount,
    readError: row.readError,
    excluded: row.excluded,
    hardlink: row.hardlink,
  })}${trashed ? ", in the Trash" : ""}`;

  return openable ? (
    // oxlint-disable-next-line jsx-a11y/click-events-have-key-events -- the listbox owns the keys
    <div
      id={rowElementId(index)}
      role="option"
      aria-selected={selected}
      aria-label={label}
      // Reachable programmatically, never a tab stop: the listbox is the single
      // stop, and Tab must leave the list rather than walk 100,000 rows.
      tabIndex={-1}
      onClick={onSelect}
      onDoubleClick={onOpen}
      className={`${shared} hover:bg-muted/60 cursor-pointer`}
    >
      {content}
    </div>
  ) : (
    // oxlint-disable-next-line jsx-a11y/click-events-have-key-events -- the listbox owns the keys
    <div
      id={rowElementId(index)}
      role="option"
      aria-selected={selected}
      aria-label={label}
      tabIndex={-1}
      onClick={onSelect}
      className={`${shared} cursor-default`}
    >
      {content}
    </div>
  );
}
