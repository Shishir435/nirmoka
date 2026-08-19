import { ChevronDown, ChevronRight, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { SystemStatus } from "@nirmoka/transport";

import { EmptyState, MetricCard, SectionTitle } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useApp } from "@/lib/app-context";
import { formatBytes } from "@/lib/format";

const percent = (value: number) => `${Math.round(value)}%`;
const temperature = (value: number | null) => (value == null ? "Unavailable" : `${value}°C`);

/**
 * Mole's `mo status`, folded in below the scan it has nothing to do with.
 *
 * It was a top-level tab reporting battery health and fan speed, which is not
 * disk cleanup — see ADR 0026. Collapsed by default and loaded on first open, so
 * visiting Storage does not run a backend command nobody asked for.
 */
export function SystemSection() {
  const { transport } = useApp();
  const [open, setOpen] = useState(false);
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [loading, setLoading] = useState(false);
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
    if (open && !status && !loading && !error) load();
    return () => {
      requestGeneration.current += 1;
    };
  }, [error, load, loading, open, status]);

  return (
    <Card className="shadow-none">
      <CardContent className="p-5">
        <div className="flex items-center justify-between gap-3">
          <button
            type="button"
            onClick={() => setOpen((value) => !value)}
            aria-expanded={open}
            className="flex items-center gap-2 text-sm font-medium"
          >
            {open ? (
              <ChevronDown className="size-4 text-muted-foreground" />
            ) : (
              <ChevronRight className="size-4 text-muted-foreground" />
            )}
            System status
            <span className="text-xs font-normal text-muted-foreground">
              {status ? `${status.healthScore}/100 · ${status.hardware.model}` : "Reported by Mole"}
            </span>
          </button>
          {open && (
            <Button variant="outline" size="sm" onClick={load} disabled={loading}>
              <RefreshCw className={loading ? "animate-spin" : ""} />
              Refresh
            </Button>
          )}
        </div>

        {open && (
          <div className="mt-5 space-y-5">
            {error ? (
              <EmptyState
                title="System status unavailable"
                text={`${error}. Install a supported Mole release, then refresh.`}
              />
            ) : !status ? (
              <EmptyState
                title="Reading system status"
                text="Mole is collecting one local snapshot."
              />
            ) : (
              <>
                {status.backendInsteadOf && (
                  <p className="rounded-lg border border-warning/30 bg-warning/10 p-3 text-xs text-warning-foreground">
                    {status.backendInsteadOf} was preferred, but {status.backend} provides system
                    status.
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

                <div>
                  <SectionTitle title="Hardware" />
                  <div className="grid grid-cols-2 gap-x-8 gap-y-3 text-sm max-[900px]:grid-cols-1">
                    <Fact label="Mac" value={status.hardware.model} />
                    <Fact label="Processor" value={status.hardware.cpuModel} />
                    <Fact label="Memory" value={status.hardware.totalRam} />
                    <Fact label="Storage" value={status.hardware.diskSize} />
                    <Fact label="macOS" value={status.hardware.osVersion} />
                    <Fact label="Host" value={status.host} />
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-8 max-[950px]:grid-cols-1">
                  <div>
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
                            {formatBytes(disk.used)} of {formatBytes(disk.total)} ·{" "}
                            {disk.filesystem}
                            {disk.external ? " · external" : ""} · SMART {disk.smartStatus}
                          </p>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div>
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
                  </div>
                </div>

                <p className="text-xs text-muted-foreground">
                  Snapshot from {status.backend} at {new Date(status.collectedAt).toLocaleString()}.
                  Nothing leaves this Mac.
                </p>
              </>
            )}
          </div>
        )}
      </CardContent>
    </Card>
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
