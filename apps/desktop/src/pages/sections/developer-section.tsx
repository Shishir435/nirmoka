import { useEffect, useMemo, useState } from "react";
import type { DeveloperCategory, DeveloperInventory, ScanSummary } from "@nirmoka/transport";

import { EmptyState, MetricCard, SafetyBanner, SectionTitle } from "@/components/shared";
import { Card, CardContent } from "@/components/ui/card";
import { useApp } from "@/lib/app-context";
import { formatBytes, formatCount } from "@/lib/format";

const labels: Record<DeveloperCategory, string> = {
  xcodeDerivedData: "Xcode Derived Data",
  simulatorData: "Simulator Data",
  xcodeArchives: "Xcode Archives",
  developerCaches: "Developer Caches & Logs",
  gitRepository: "Git Repositories",
  nodeModules: "node_modules",
};

/**
 * The same scan tree, filtered to explicit developer evidence. A view of the
 * scan rather than a destination of its own — see ADR 0026.
 */
export function DeveloperSection({ summary }: { summary: ScanSummary }) {
  const { transport } = useApp();
  const [inventory, setInventory] = useState<DeveloperInventory | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setError(null);
    transport.developerInventory(summary.scanId).then(
      (value) => live && setInventory(value),
      (reason: unknown) => live && setError(String(reason)),
    );
    return () => {
      live = false;
    };
  }, [summary, transport]);

  const totals = useMemo(() => {
    const value = new Map<DeveloperCategory, { bytes: number; count: number }>();
    for (const row of inventory?.rows ?? []) {
      const current = value.get(row.category) ?? { bytes: 0, count: 0 };
      value.set(row.category, { bytes: current.bytes + row.totalBytes, count: current.count + 1 });
    }
    return value;
  }, [inventory]);
  const totalBytes = [...totals.values()].reduce((sum, value) => sum + value.bytes, 0);

  if (error) return <p className="text-sm text-destructive">{error}</p>;

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-4 gap-3 max-[1100px]:grid-cols-2">
        {["xcodeDerivedData", "gitRepository", "nodeModules"].map((category) => {
          const value = totals.get(category as DeveloperCategory) ?? { bytes: 0, count: 0 };
          return (
            <MetricCard
              key={category}
              label={labels[category as DeveloperCategory]}
              value={formatBytes(value.bytes)}
              hint={`${formatCount(value.count)} found`}
            />
          );
        })}
        <MetricCard
          label="Total matched"
          value={formatBytes(totalBytes)}
          hint={`${formatCount(inventory?.total ?? 0)} entries`}
        />
      </div>
      <Card className="shadow-none">
        <CardContent className="p-5">
          <SectionTitle title="Largest developer data" />
          {(inventory?.rows.length ?? 0) === 0 ? (
            <EmptyState
              title="No known developer paths found"
              text="Results are based only on explicit path/name evidence; Nirmoka does not guess project data."
            />
          ) : (
            <div className="overflow-hidden rounded-lg border">
              <div className="grid grid-cols-[160px_minmax(0,1fr)_100px_100px] gap-3 bg-muted/50 px-3 py-2 text-[11px] font-medium text-muted-foreground">
                <span>Category</span>
                <span>Path</span>
                <span>Size</span>
                <span>Modified</span>
              </div>
              {inventory?.rows.map((row) => (
                <div
                  key={`${row.category}-${row.id}`}
                  className="grid grid-cols-[160px_minmax(0,1fr)_100px_100px] gap-3 border-t px-3 py-3 text-xs"
                >
                  <span>{labels[row.category]}</span>
                  <span className="truncate font-mono text-[11px]">{row.path}</span>
                  <span>
                    {formatBytes(row.totalBytes)}
                    {row.sizeIsPartial ? " +" : ""}
                  </span>
                  <span className="text-muted-foreground">
                    {row.modifiedAtMs == null
                      ? "Unavailable"
                      : new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(
                          row.modifiedAtMs,
                        )}
                  </span>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
      <SafetyBanner compact>
        <p className="text-xs text-muted-foreground">
          Sizes come from the scanner. Modified dates remain unavailable because the ncdu wire
          format does not contain them. Switch to Folders to move any of these to the Trash.
        </p>
      </SafetyBanner>
    </div>
  );
}
