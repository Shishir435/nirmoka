import { Brush, RotateCcw, Trash2, Undo2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { DeleteOperation } from "@nirmoka/transport";

import { EmptyState, MetricCard, PageHeader, SafetyBanner, StatusBadge } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useApp } from "@/lib/app-context";
import {
  activityCounts,
  measuredBytes,
  mergeActivity,
  NO_JOURNALS,
  recoveryOf,
  type ActivityEntry,
  type Journals,
} from "@/lib/engine/activity-feed";
import { outcomeLabel, outcomeTone } from "@/lib/engine/cleanup-flow";
import { formatBytes, formatCount } from "@/lib/format";

const dates = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export function ActivityPage() {
  const { transport } = useApp();
  const [journals, setJournals] = useState<Journals | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    setError(null);
    // One await per journal, and a failure names itself rather than emptying the
    // page: a cleanup log that cannot be read is not evidence that nothing was
    // trashed.
    Promise.allSettled([
      transport.trashLog(),
      transport.cleanupLog(),
      transport.operationLog(),
    ]).then(([trashed, cleaned, deleted]) => {
      setJournals({
        trashed: trashed.status === "fulfilled" ? trashed.value : [],
        cleaned: cleaned.status === "fulfilled" ? cleaned.value : [],
        deleted: deleted.status === "fulfilled" ? deleted.value : [],
      });
      const failure = [trashed, cleaned, deleted].find((result) => result.status === "rejected");
      if (failure?.status === "rejected") setError(String(failure.reason));
    });
  }, [transport]);

  useEffect(load, [load]);

  const undo = async (operation: DeleteOperation) => {
    try {
      await transport.undoDelete(operation.id);
      load();
    } catch (reason) {
      setError(String(reason));
    }
  };

  const entries = mergeActivity(journals ?? NO_JOURNALS);
  const counts = activityCounts(entries);

  return (
    <div className="space-y-6">
      <PageHeader
        title="Activity"
        subtitle="Everything this Mac's journal records, newest first"
        action={
          <Button variant="outline" onClick={load}>
            <RotateCcw />
            Refresh
          </Button>
        }
      />
      {error && <p className="text-sm text-destructive">{error}</p>}

      {counts.total > 0 && (
        <div className="grid grid-cols-4 gap-3 max-[1100px]:grid-cols-2">
          <MetricCard
            label="Moved to Trash"
            value={formatCount(counts.trashed)}
            hint={`${formatBytes(measuredBytes(entries))} measured before the move`}
          />
          <MetricCard
            label="Cleanup runs"
            value={formatCount(counts.cleaned)}
            hint="Sizes are Mole's reviewed totals, not receipts"
          />
          <MetricCard
            label="Deletions"
            value={formatCount(counts.deleted)}
            hint="Recorded backend receipts"
          />
          <MetricCard
            label="Journal entries"
            value={formatCount(counts.total)}
            hint="One shared id space"
          />
        </div>
      )}

      {counts.total === 0 ? (
        <EmptyState
          title={journals ? "Nothing has happened yet" : "Loading activity"}
          text="Scans are not recorded, because nothing changed. Trashed items, cleanup runs, and deletions appear here as they happen."
        />
      ) : (
        <Card className="shadow-none">
          <CardContent className="p-0">
            <div className="divide-y">
              {entries.map((entry) => (
                <ActivityRow key={`${entry.kind}-${entry.id}`} entry={entry} onUndo={undo} />
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      <SafetyBanner compact>
        <p className="text-xs text-muted-foreground">
          The journal is stored on this Mac and never leaves it. A trashed item is restored from the
          Finder with Put Back; a cleanup run has no per-path receipt to restore from, because Mole
          publishes none.
        </p>
      </SafetyBanner>
    </div>
  );
}

function ActivityRow({
  entry,
  onUndo,
}: {
  entry: ActivityEntry;
  onUndo: (operation: DeleteOperation) => void;
}) {
  const recovery = recoveryOf(entry);

  return (
    <div className="flex items-start gap-3 px-4 py-3 text-sm">
      <span className="mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg bg-muted text-muted-foreground">
        {entry.kind === "cleaned" ? <Brush className="size-4" /> : <Trash2 className="size-4" />}
      </span>
      <div className="min-w-0 flex-1">
        <p className="flex flex-wrap items-baseline gap-x-2">
          <span className="font-medium">{title(entry)}</span>
          <span className="text-xs text-muted-foreground">{dates.format(entry.atMs)}</span>
        </p>
        <p className="mt-0.5 truncate text-xs text-muted-foreground">{detail(entry)}</p>
        {entry.operation.logError && (
          <p className="mt-1 text-xs text-warning-foreground">
            This happened but could not be written to the journal: {entry.operation.logError}
          </p>
        )}
      </div>
      <div className="shrink-0 pt-0.5">
        {entry.kind === "cleaned" ? (
          <StatusBadge tone={outcomeTone(entry.operation.completion)}>
            {outcomeLabel(entry.operation.completion)}
          </StatusBadge>
        ) : recovery === "undoable" && entry.kind === "deleted" ? (
          <Button size="sm" variant="outline" onClick={() => onUndo(entry.operation)}>
            <Undo2 />
            Undo
          </Button>
        ) : (
          <StatusBadge tone={recovery === "undone" ? "neutral" : "success"}>
            {recovery === "putBack"
              ? "In the Trash"
              : recovery === "undone"
                ? "Undone"
                : "Complete"}
          </StatusBadge>
        )}
      </div>
    </div>
  );
}

function title(entry: ActivityEntry): string {
  switch (entry.kind) {
    case "trashed":
      return name(entry.operation.targetPath);
    case "deleted":
      return name(entry.operation.targetPath);
    case "cleaned":
      return `${entry.operation.backend} cleanup`;
  }
}

function detail(entry: ActivityEntry): string {
  switch (entry.kind) {
    case "trashed":
      return `Moved to the Trash · ${formatBytes(entry.operation.totalBytes)} · ${entry.operation.targetPath}`;
    case "deleted":
      return `${entry.operation.disposition === "trash" ? "Trashed" : "Deleted"} by ${entry.operation.backend} · ${entry.operation.targetPath}`;
    case "cleaned":
      return `${entry.operation.backend} ${entry.operation.backendVersion} · reviewed ${formatCount(entry.operation.reviewedItems)} items${
        entry.operation.reviewedPotentialCleanup
          ? ` and ${entry.operation.reviewedPotentialCleanup}`
          : ""
      } · Mole re-discovered candidates as it ran`;
  }
}

function name(path: string): string {
  // The last segment, not the first, so `find` is the wrong shape. `findLast`
  // is the right one and is ES2023; the lib here is pinned to ES2022 because
  // that is the macOS 12 WebView floor, so this stays an index.
  const segments = path.split("/").filter(Boolean);
  return segments[segments.length - 1] ?? path;
}
