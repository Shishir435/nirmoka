import { ScanLine, Square } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { formatCount, plural } from "@/lib/format";

export function ScanControls({ compact = false }: { compact?: boolean }) {
  const { selection, listenersReady, scan, startScan, cancelScan, backendError } = useApp();
  const [path, setPath] = useState("~");
  const canScan = selection?.scanner != null && listenersReady;

  return (
    <div className={compact ? "space-y-2" : "space-y-3 rounded-xl border bg-card p-4"}>
      <div className="flex gap-2">
        <input
          aria-label="Directory to scan"
          value={path}
          onChange={(event) => setPath(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && canScan && scan.status !== "scanning") {
              void startScan(path);
            }
          }}
          disabled={scan.status === "scanning"}
          placeholder="Directory to scan"
          spellCheck={false}
          className="h-9 min-w-48 flex-1 rounded-md border bg-background px-3 font-mono text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/20 disabled:opacity-50"
        />
        {scan.status === "scanning" ? (
          <Button variant="destructive" onClick={() => void cancelScan()}>
            <Square /> Stop
          </Button>
        ) : (
          <Button disabled={!canScan || !path.trim()} onClick={() => void startScan(path)}>
            <ScanLine /> Scan
          </Button>
        )}
      </div>
      {scan.status === "scanning" && (
        <div className="text-xs text-muted-foreground">
          <p>Scanning {plural(scan.progress.scanned, "entry", "entries")}…</p>
          <p className="truncate font-mono">{scan.progress.currentPath}</p>
        </div>
      )}
      {scan.status === "cancelled" && <p className="text-xs">Scan cancelled.</p>}
      {scan.status === "failed" && <p className="text-xs text-destructive">{scan.message}</p>}
      {!selection?.scanner && (
        <p className="text-xs text-muted-foreground">
          No supported scanner is installed. Install ncdu 2.x, then refresh backend detection.
        </p>
      )}
      {backendError && <p className="text-xs text-destructive">{backendError}</p>}
      {scan.status === "done" && !compact && (
        <p className="text-xs text-muted-foreground">
          Completed with {scan.summary.backendId} · {formatCount(scan.summary.entries)} entries
        </p>
      )}
    </div>
  );
}
