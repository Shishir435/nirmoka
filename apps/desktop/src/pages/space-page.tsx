import { useState } from "react";
import type { Sort } from "@nirmoka/transport";

import { ScanControls } from "@/components/scan-controls";
import { EmptyState, PageHeader, SafetyBanner } from "@/components/shared";
import { TreeView } from "@/components/tree-view";
import { useApp } from "@/lib/app-context";

export function SpacePage() {
  const { transport, scan } = useApp();
  const [parentId, setParentId] = useState<number | null>(null);
  const [sort, setSort] = useState<Sort>("largestFirst");
  const summary = scan.status === "done" ? scan.summary : null;

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
          onNavigate={setParentId}
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
