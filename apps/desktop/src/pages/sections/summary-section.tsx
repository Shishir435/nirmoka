import { Folder } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { Row, ScanSummary, VolumeInfo } from "@nirmoka/transport";

import { ChartLegend, DonutChart, MetricCard, SectionTitle } from "@/components/shared";
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

/**
 * What the completed scan adds up to, above the tree it came from. This was the
 * Overview page, whose only unique content was these numbers and a button that
 * navigated to the tree — see ADR 0026.
 */
export function SummarySection({ summary }: { summary: ScanSummary }) {
  const { transport } = useApp();
  const [rows, setRows] = useState<Row[]>([]);
  const [rowsError, setRowsError] = useState<string | null>(null);
  const [volume, setVolume] = useState<VolumeInfo | null>(null);
  const [volumeError, setVolumeError] = useState<string | null>(null);

  useEffect(() => {
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
    <>
      <div className="grid grid-cols-4 gap-3 max-[1100px]:grid-cols-2">
        <MetricCard
          label="Scanned Size"
          value={formatBytes(summary.totalBytes)}
          hint={`${formatCount(summary.entries)} entries · ${formatCount(summary.directories)} directories`}
        />
        <MetricCard
          label="Volume Used"
          value={volume ? formatBytes(volume.usedBytes) : "Unavailable"}
          hint={volume ? `${formatBytes(volume.freeBytes)} free` : "Separate from scanned size"}
        />
        <MetricCard
          label="Volume Capacity"
          value={volume ? formatBytes(volume.totalBytes) : "Unavailable"}
          hint={volume?.mountPoint ?? volumeError ?? "Reading volume"}
        />
        <MetricCard
          label="Scanner"
          value={summary.backendId}
          hint={summary.backendVersion ?? "Version unavailable"}
        />
      </div>
      <Card className="shadow-none">
        <CardContent className="p-5">
          <SectionTitle title="Storage breakdown" />
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
  );
}
