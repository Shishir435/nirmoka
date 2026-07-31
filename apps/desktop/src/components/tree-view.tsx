/**
 * The directory browser.
 *
 * Rows are virtualized from the first commit rather than after it got slow. The
 * DOM holds the rows on screen and a little either side; the rest of the
 * directory stays in Rust and arrives a chunk at a time as it is scrolled to.
 * A directory of a hundred thousand entries rendered as a hundred thousand
 * `<li>` elements is the failure that gets blamed on the GUI framework.
 */

import { useEffect, useRef } from "react";

import type { Row, ScanSummary, Sort, Transport } from "@nirmoka/transport";
import { useVirtualizer } from "@tanstack/react-virtual";

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
  onSort,
}: {
  transport: Transport;
  summary: ScanSummary;
  parentId: number | null;
  sort: Sort;
  onNavigate: (id: number | null) => void;
  onSort: (sort: Sort) => void;
}) {
  const directory = useDirectory(transport, { scanId: summary.scanId, parentId, sort });
  const scroller = useRef<HTMLDivElement>(null);

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

  // A new directory or a new order starts at the top. The scroll container
  // outlives both, so without this, opening a directory from row 400 lands 400
  // rows into the one it opened.
  useEffect(() => {
    if (scroller.current) scroller.current.scrollTop = 0;
  }, [parentId, sort]);

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

  const { header, rowAt } = directory;
  const up = header.ancestors.at(-1);

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Breadcrumb header={header} onNavigate={onNavigate} />
        <SortControls sort={header.sort} onSort={onSort} />
      </div>

      <div className="text-muted-foreground flex items-center gap-3 text-xs">
        <span className="truncate font-mono">{header.path}</span>
        <span>{plural(header.total, "entry", "entries")}</span>
      </div>

      {header.total === 0 ? (
        <p className="text-muted-foreground rounded-lg border border-dashed px-4 py-8 text-center text-sm">
          {header.readError
            ? "This directory could not be read. Its size is a lower bound, and what is inside it is unknown — usually a permissions problem."
            : "Nothing here."}
        </p>
      ) : (
        <div
          ref={scroller}
          className="h-[26rem] overflow-auto rounded-lg border"
          // The list is a scroll region of its own, so it takes focus for
          // keyboard scrolling rather than moving the page behind it.
          tabIndex={0}
        >
          <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
            {items.map((item) => {
              const row = rowAt(item.index);

              return (
                <div
                  key={item.key}
                  className="absolute top-0 left-0 w-full"
                  style={{ height: `${item.size}px`, transform: `translateY(${item.start}px)` }}
                >
                  {row ? <RowLine row={row} onOpen={() => onNavigate(row.id)} /> : <Placeholder />}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {up !== undefined && (
        <Button variant="outline" size="sm" onClick={() => onNavigate(up.id)}>
          Up to {up.name}
        </Button>
      )}
    </div>
  );
}

function RowLine({ row, onOpen }: { row: Row; onOpen: () => void }) {
  // Every directory opens, including the ones with nothing in them. Gating on
  // `childCount > 0` would make the two states worth distinguishing — empty,
  // and unreadable — the two the user cannot reach: an unreadable directory has
  // no children precisely because it could not be read, and refusing to open it
  // leaves the reason unsaid.
  const openable = row.kind === "directory";

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
      <span className="w-20 shrink-0 text-right font-mono text-sm tabular-nums">
        {formatBytes(row.totalBytes)}
      </span>
    </>
  );

  const shared = "border-border/60 flex h-9 w-full items-center gap-3 border-b px-4";

  return openable ? (
    <button type="button" onClick={onOpen} className={`${shared} hover:bg-muted/60 cursor-pointer`}>
      {content}
    </button>
  ) : (
    <div className={shared}>{content}</div>
  );
}
