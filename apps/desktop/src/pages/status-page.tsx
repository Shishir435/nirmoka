import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { SystemStatus } from "@nirmoka/transport";

import { EmptyState, MetricCard, PageHeader, SectionTitle } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useApp } from "@/lib/app-context";
import { formatBytes } from "@/lib/format";

const percent = (value: number) => `${Math.round(value)}%`;
const temperature = (value: number | null) => (value == null ? "Unavailable" : `${value}°C`);

export function StatusPage() {
  const { transport } = useApp();
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestGeneration = useRef(0);

  const load = useCallback(() => {
    const generation = ++requestGeneration.current;
    setLoading(true);
    setError(null);
    transport
      .systemStatus()
      .then(
        (value) => {
          if (generation === requestGeneration.current) setStatus(value);
        },
        (reason: unknown) => {
          if (generation === requestGeneration.current) setError(String(reason));
        },
      )
      .finally(() => {
        if (generation === requestGeneration.current) setLoading(false);
      });
  }, [transport]);

  useEffect(() => {
    load();
    return () => {
      requestGeneration.current += 1;
    };
  }, [load]);

  return (
    <div className="space-y-6">
      <PageHeader
        title="System Status"
        subtitle="Live facts reported by Mole"
        action={
          <Button variant="outline" onClick={load} disabled={loading}>
            <RefreshCw className={loading ? "animate-spin" : ""} />
            Refresh
          </Button>
        }
      />

      {error ? (
        <EmptyState
          title="System status unavailable"
          text={`${error}. Install a supported Mole release, then refresh.`}
        />
      ) : !status ? (
        <EmptyState title="Reading system status" text="Mole is collecting one local snapshot." />
      ) : (
        <>
          {status.backendInsteadOf && (
            <p className="rounded-lg border border-warning/30 bg-warning/10 p-3 text-xs text-warning-foreground">
              {status.backendInsteadOf} was preferred, but {status.backend} provides system status.
            </p>
          )}

          <div className="grid grid-cols-4 gap-3 max-[1100px]:grid-cols-2">
            <MetricCard
              label="Health"
              value={`${status.healthScore}/100`}
              hint={status.healthScoreMessage}
            />
            <MetricCard
              label="CPU"
              value={percent(status.cpu.usage)}
              hint={`${status.cpu.logicalCpu} logical cores`}
            />
            <MetricCard
              label="Memory"
              value={percent(status.memory.usedPercent)}
              hint={`${formatBytes(status.memory.available)} available · ${status.memory.pressure}`}
            />
            <MetricCard label="Uptime" value={status.uptime} hint={status.hardware.model} />
          </div>

          <Card className="shadow-none">
            <CardContent className="p-5">
              <SectionTitle title="Hardware" />
              <div className="grid grid-cols-2 gap-x-8 gap-y-3 text-sm max-[900px]:grid-cols-1">
                <Fact label="Mac" value={status.hardware.model} />
                <Fact label="Processor" value={status.hardware.cpuModel} />
                <Fact label="Memory" value={status.hardware.totalRam} />
                <Fact label="Storage" value={status.hardware.diskSize} />
                <Fact label="macOS" value={status.hardware.osVersion} />
                <Fact label="Host" value={status.host} />
              </div>
            </CardContent>
          </Card>

          <div className="grid grid-cols-2 gap-4 max-[950px]:grid-cols-1">
            <Card className="shadow-none">
              <CardContent className="p-5">
                <SectionTitle title="Disks" />
                <div className="divide-y">
                  {status.disks.map((disk) => (
                    <div key={`${disk.device}-${disk.mount}`} className="py-3 text-sm">
                      <div className="flex items-center justify-between gap-3">
                        <span className="truncate font-mono">{disk.mount}</span>
                        <span className="font-medium tabular-nums">
                          {percent(disk.usedPercent)}
                        </span>
                      </div>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {formatBytes(disk.used)} of {formatBytes(disk.total)} · {disk.filesystem}
                        {disk.external ? " · external" : ""} · SMART {disk.smartStatus}
                      </p>
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>

            <Card className="shadow-none">
              <CardContent className="p-5">
                <SectionTitle title="Power & Temperature" />
                <div className="space-y-3 text-sm">
                  {status.batteries.map((battery, index) => (
                    <Fact
                      key={index}
                      label={`Battery ${index + 1}`}
                      value={`${percent(battery.percent)} · ${battery.status} · ${battery.health}`}
                    />
                  ))}
                  <Fact label="CPU temperature" value={temperature(status.thermal.cpuTemp)} />
                  <Fact label="GPU temperature" value={temperature(status.thermal.gpuTemp)} />
                  <Fact
                    label="Fan"
                    value={
                      status.thermal.fanSpeed == null
                        ? "Unavailable"
                        : `${Math.round(status.thermal.fanSpeed).toLocaleString()} RPM`
                    }
                  />
                </div>
              </CardContent>
            </Card>
          </div>

          <p className="text-xs text-muted-foreground">
            Snapshot from {status.backend} at {new Date(status.collectedAt).toLocaleString()}.
            Nothing leaves this Mac.
          </p>
        </>
      )}
    </div>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-border/50 pb-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="min-w-0 truncate text-right font-medium">{value}</span>
    </div>
  );
}
