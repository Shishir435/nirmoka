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

import { Eye, FolderOpen } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { Row, ScanSummary, Sort, Transport } from "@nirmoka/transport";
import { useVirtualizer } from "@tanstack/react-virtual";

import { rowIntent, rowLabel } from "@/components/row-keyboard";
import { Button } from "@/components/ui/button";
import { useDirectory, type DirectoryHeader } from "@/hooks/use-directory";
import { formatBytes, formatCount, plural } from "@/lib/format";

/** Row height in pixels. The virtualizer needs this before it measures. */
const ROW_HEIGHT = 36;

const SORTS: { value: Sort; label: string }[] = [
  { value: "largestFirst", label: "Largest" },
  { value: "smallestFirst", label: "Smallest" },
  { value: "nameAscending", label: "Name A–Z" },
  { value: "nameDescending", label: "Name Z–A" },
];

/** Stable id for a row, so `aria-activedescendant` can name one. */
const rowElementId = (index: number) => `directory-row-${index}`;

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
 * the real row lands.
 */
function Placeholder() {
  return <div className="bg-muted/40 mx-4 my-3 h-3 animate-pulse rounded" />;
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
  const [features, setFeatures] = useState<{ revealLabel: string; quickLook: boolean } | null>(
    null,
  );
  const [actionError, setActionError] = useState<string | null>(null);

  const total = directory.status === "ready" ? directory.header.total : 0;

  const virtualizer = useVirtualizer({
    count: total,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  const items = virtualizer.getVirtualItems();
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
  }, [parentId, sort]);

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

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    // A modified key belongs to the platform: ⌘↓ and friends are not ours.
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
          </>
        )}
      </div>

      <div className="text-muted-foreground flex items-center gap-3 text-xs">
        <span className="truncate font-mono">{header.path}</span>
        <span>{plural(header.total, "entry", "entries")}</span>
      </div>

      {actionError && <p className="text-destructive text-xs">{actionError}</p>}

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
          aria-activedescendant={selected === null ? undefined : rowElementId(selected)}
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
                      onSelect={() => setSelected(item.index)}
                      onOpen={() => onNavigate(row.id)}
                    />
                  ) : (
                    <Placeholder />
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

function RowLine({
  row,
  index,
  selected,
  onSelect,
  onOpen,
}: {
  row: Row;
  index: number;
  selected: boolean;
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
      <span className="flex-1 truncate text-left font-mono text-sm">
        {row.name}
        {row.kind === "directory" ? "/" : ""}
      </span>
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
  const label = rowLabel({
    name: row.name,
    kind: row.kind,
    size,
    childCount: row.childCount,
    readError: row.readError,
    excluded: row.excluded,
    hardlink: row.hardlink,
  });

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
