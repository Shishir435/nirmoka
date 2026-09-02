import { useEffect, useState } from "react";
import type { Sort } from "@nirmoka/transport";

import { StartScreen } from "@/components/start-screen";
import { Button } from "@/components/ui/button";
import { TreeView } from "@/components/tree-view";
import { useApp } from "@/lib/app-context";
import { STORAGE_VIEWS, type StorageView } from "@/lib/engine/route";
import { ApplicationsSection } from "@/pages/sections/applications-section";
import { DeveloperSection } from "@/pages/sections/developer-section";
import { InspectorPage } from "@/pages/inspector-page";
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
  inspect,
  onView,
  onInspect,
}: {
  /** `null` is the dashboard. A view is the browser beneath it — see ADR 0031. */
  view: StorageView | null;
  /** The application being inspected, by node id, or `null` for none. */
  inspect: number | null;
  onView: (view: StorageView | null) => void;
  onInspect: (nodeId: number) => void;
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

  // Before a scan there is no tree to view, no summary to head, and no reason to
  // ask Mole about system status. Capacity is the one thing that can be shown
  // without a backend, so that is the whole screen.
  if (!summary) return <StartScreen />;

  // The dashboard is the destination. Everything it lists is a way into the
  // tree it came from, and that tree is a screen of its own rather than a
  // section stacked underneath — ADR 0031.
  // An application replaces the screen rather than sitting inside it, which is
  // what every screen below the dashboard does — ADR 0031.
  //
  // Tested for a number rather than against `null`: `undefined !== null` is
  // true, so a prop that failed to arrive would open the Inspector on nothing
  // and send `undefined` across the IPC boundary as a node id.
  if (Number.isInteger(inspect)) {
    return <InspectorPage nodeId={inspect as number} onBack={() => onView(null)} />;
  }

  if (view === null) {
    return (
      <SummarySection
        summary={summary}
        onOpen={(nodeId) => {
          open(nodeId);
          onView("folders");
        }}
        onInspect={onInspect}
      />
    );
  }

  return (
    <div className="space-y-6">
      {/* The rule spans the content width; the row inside it is pulled left by
          the buttons' own padding, so "Folders" starts under the content edge
          rather than a button's worth of padding inside it. */}
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

      {/* Applications keeps its second source — Mole reports what is installed
          whether or not anything has been scanned. */}
      {view === "applications" ? (
        <ApplicationsSection />
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
