import { useEffect, useState } from "react";
import type { Sort } from "@nirmoka/transport";

import { EmptyState, PageHeader } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { TreeView } from "@/components/tree-view";
import { useApp } from "@/lib/app-context";
import { STORAGE_VIEWS, type StorageView } from "@/lib/engine/route";
import { ApplicationsSection } from "@/pages/sections/applications-section";
import { DeveloperSection } from "@/pages/sections/developer-section";
import { SummarySection } from "@/pages/sections/summary-section";
import { SystemSection } from "@/pages/sections/system-section";
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

const viewLabels: Record<StorageView, string> = {
  folders: "Folders",
  developer: "Developer",
  applications: "Applications",
};

/**
 * Everything derived from one scan, in one place: the numbers it adds up to, and
 * a view over the tree those numbers came from. Overview, Space Explorer,
 * Developer, Applications, and System Status were five tabs reading this one
 * scan — see ADR 0026.
 */
export function StoragePage({
  view,
  onView,
}: {
  view: StorageView;
  onView: (view: StorageView) => void;
}) {
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
      <PageHeader
        title="Storage"
        subtitle={
          summary
            ? "Everything below comes from the scan in the bar above"
            : "Scan a directory to see what is using space"
        }
      />

      {summary && <SummarySection summary={summary} />}

      {/* The rule spans the content width; the row inside it is pulled left by
          the buttons' own padding, so "Folders" starts under "Storage" rather
          than a button's worth of padding inside it. */}
      <div className="border-b pb-2">
        <div className="-ml-3 flex flex-wrap items-center gap-1" role="tablist">
          {STORAGE_VIEWS.map((candidate) => (
            <Button
              key={candidate}
              role="tab"
              aria-selected={view === candidate}
              variant={view === candidate ? "secondary" : "ghost"}
              size="sm"
              onClick={() => onView(candidate)}
            >
              {viewLabels[candidate]}
            </Button>
          ))}
        </div>
      </div>

      {/* Applications is the one view with a source other than the scan: Mole
          reports what is installed whether or not anything has been scanned. The
          other two are the tree, so they wait for it. */}
      {view === "applications" ? (
        <ApplicationsSection />
      ) : !summary ? (
        <EmptyState
          title="No completed scan"
          text="Type a directory in the bar above and press Scan. Your home directory is the default, and scanning is read-only."
        />
      ) : view === "folders" ? (
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
        <DeveloperSection summary={summary} />
      )}

      <SystemSection />
    </div>
  );
}
