import { Folder, Play } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { Row, VolumeInfo } from "@nirmoka/transport";

import { ScanControls } from "@/components/scan-controls";
import {
  ChartLegend,
  DonutChart,
  EmptyState,
  MetricCard,
  PageHeader,
  SafetyBanner,
  SectionTitle,
} from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useApp } from "@/lib/app-context";
import { formatBytes, formatCount } from "@/lib/format";

const colors = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
];

export function OverviewPage() {
  const { transport, scan } = useApp();
  const summary = scan.status === "done" ? scan.summary : null;
  const [rows, setRows] = useState<Row[]>([]);
  const [rowsError, setRowsError] = useState<string | null>(null);
  const [volume, setVolume] = useState<VolumeInfo | null>(null);
  const [volumeError, setVolumeError] = useState<string | null>(null);

  useEffect(() => {
    if (!summary) {
      setRows([]);
      return;
    }
    let live = true;
    setRowsError(null);
    transport.rows(summary.scanId, null, "largestFirst", 0, 100).then(
      (page) => live && setRows(page.rows),
      (error: unknown) => live && setRowsError(String(error)),
    );
    setVolumeError(null);
    transport.volumeInfo(summary.rootPath).then(
      (value) => live && setVolume(value),
      (error: unknown) => live && setVolumeError(String(error)),
    );
    return () => {
      live = false;
    };
  }, [summary, transport]);

  const breakdown = useMemo(() => {
    if (!summary) return [];
    const top = rows.slice(0, 5).map((row, index) => ({
      name: row.name,
      value: row.totalBytes,
      color: colors[index % colors.length]!,
    }));
    const shown = top.reduce((total, item) => total + item.value, 0);
    const remaining = Math.max(0, summary.totalBytes - shown);
    return remaining > 0
      ? [
          ...top,
          { name: "Other scanned entries", value: remaining, color: "var(--muted-foreground)" },
        ]
      : top;
  }, [rows, summary]);

  return (
    <div className="space-y-6">
      <PageHeader title="Overview" subtitle="Facts from the current scan" />
      <ScanControls />
      {!summary ? (
        <EmptyState
          title="No completed scan"
          text="Choose a directory above. Your home directory is the default; the scan is read-only."
        />
      ) : (
        <>
          <div className="grid grid-cols-3 gap-3 max-[1100px]:grid-cols-2">
            <MetricCard
              label="Scanned Size"
              value={formatBytes(summary.totalBytes)}
              hint="Not disk capacity"
            />
            <MetricCard
              label="Volume Capacity"
              value={volume ? formatBytes(volume.totalBytes) : "Unavailable"}
              hint={volume?.mountPoint ?? volumeError ?? "Reading volume"}
            />
            <MetricCard
              label="Volume Used"
              value={volume ? formatBytes(volume.usedBytes) : "Unavailable"}
              hint={volume ? `${formatBytes(volume.freeBytes)} free` : "Separate from scanned size"}
            />
            <MetricCard
              label="Entries"
              value={formatCount(summary.entries)}
              hint={`${formatCount(summary.directories)} directories`}
            />
            <MetricCard
              label="Root"
              value={summary.rootPath.split("/").filter(Boolean).at(-1) ?? summary.rootPath}
              hint={summary.rootPath}
            />
            <MetricCard
              label="Scanner"
              value={summary.backendId}
              hint={summary.backendVersion ?? "Version unavailable"}
            />
          </div>
          <Card className="shadow-none">
            <CardContent className="p-5">
              <SectionTitle
                title="Storage Breakdown"
                action={
                  <Button
                    variant="link"
                    size="sm"
                    onClick={() => {
                      window.location.hash = "/space";
                    }}
                  >
                    Explore <Play />
                  </Button>
                }
              />
              {rowsError ? (
                <p className="text-sm text-destructive">{rowsError}</p>
              ) : (
                <div className="grid grid-cols-[minmax(350px,1.3fr)_minmax(260px,.7fr)] gap-8 max-[1050px]:grid-cols-1">
                  <div className="flex items-center justify-center gap-10">
                    <DonutChart
                      data={breakdown}
                      center={formatBytes(summary.totalBytes)}
                      sublabel="Scanned"
                    />
                    <ChartLegend data={breakdown} />
                  </div>
                  <div>
                    <p className="mb-2 text-xs font-medium text-muted-foreground">
                      Largest entries at scan root
                    </p>
                    <div className="divide-y">
                      {rows.slice(0, 8).map((row) => (
                        <div key={row.id} className="flex items-center gap-3 py-2.5 text-sm">
                          <Folder className="size-4 text-muted-foreground" />
                          <span className="flex-1 truncate">{row.name}</span>
                          <span className="text-xs tabular-nums text-muted-foreground">
                            {formatBytes(row.totalBytes)}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
          {(summary.readErrors > 0 || summary.excluded > 0) && (
            <p className="rounded-lg border border-warning/30 bg-warning/10 p-3 text-xs text-warning-foreground">
              This total is a lower bound: {formatCount(summary.readErrors)} unreadable and{" "}
              {formatCount(summary.excluded)} excluded entries.
            </p>
          )}
        </>
      )}
      <SafetyBanner />
    </div>
  );
}
