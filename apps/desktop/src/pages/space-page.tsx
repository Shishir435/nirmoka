import { useEffect, useState } from "react";
import type { Sort } from "@nirmoka/transport";

import { ScanControls } from "@/components/scan-controls";
import { EmptyState, PageHeader, SafetyBanner } from "@/components/shared";
import { TreeView } from "@/components/tree-view";
import { useApp } from "@/lib/app-context";
import {
  canGoBack,
  canGoForward,
  currentLocation,
  EMPTY_HISTORY,
  goBack,
  goForward,
  parentIdForScan,
  visit,
  type SpaceHistory,
} from "@/pages/space-navigation";

export function SpacePage() {
  const { transport, scan } = useApp();
  const [history, setHistory] = useState<SpaceHistory>(EMPTY_HISTORY);
  const [sort, setSort] = useState<Sort>("largestFirst");
  const summary = scan.status === "done" ? scan.summary : null;
  const scanId = summary?.scanId ?? null;
  const parentId = parentIdForScan(currentLocation(history), scanId);

  // A rescan renumbers the tree from zero, so ids from the previous scan name
  // different directories. Nothing about the old history is worth keeping.
  useEffect(() => setHistory(EMPTY_HISTORY), [scanId]);

  const open = (nextParentId: number | null) => {
    if (!summary) return;
    setHistory((current) => visit(current, { scanId: summary.scanId, parentId: nextParentId }));
  };

  return (
    <div className="space-y-6">
      <PageHeader title="Space Explorer" subtitle="Paginated rows from the Rust-side scan tree" />
      <ScanControls compact />
      {summary ? (
        <TreeView
          transport={transport}
          summary={summary}
          parentId={parentId}
          sort={sort}
          onNavigate={open}
          onBack={() => setHistory(goBack)}
          onForward={() => setHistory(goForward)}
          canGoBack={canGoBack(history)}
          canGoForward={canGoForward(history)}
          onSort={setSort}
        />
      ) : (
        <EmptyState
          title="Nothing to explore yet"
          text="Complete a scan to browse real files and folders."
        />
      )}
      <SafetyBanner compact>
        <p className="text-xs text-muted-foreground">
          Folder navigation and sorting are resolved in Rust; only visible row windows cross into
          the UI.
        </p>
      </SafetyBanner>
    </div>
  );
}
