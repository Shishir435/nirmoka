import { ScanLine, Square } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { canStartScan } from "@/lib/engine/scan-machine";
import { formatCount, plural } from "@/lib/format";

/**
 * One scan bar, in the shell header. Rust holds exactly one scan at a time, so a
 * second copy of these controls implied a second scan — see ADR 0026.
 *
 * `compact` is the header layout: one row, with status beside the button rather
 * than stacked under it.
 */
export function ScanControls({ compact = false }: { compact?: boolean }) {
  const { selection, listenersReady, scan, startScan, cancelScan, backendError } = useApp();
  const [path, setPath] = useState("~");
  const canScan = canStartScan({ scanner: selection?.scanner, listenersReady, state: scan });

  return (
    <div className={compact ? "space-y-1" : "space-y-3 rounded-xl border bg-card p-4"}>
      <div className="flex items-center gap-2">
        <input
          aria-label="Directory to scan"
          value={path}
          onChange={(event) => setPath(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && canScan) {
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
            <ScanLine /> {scan.status === "done" ? "Rescan" : "Scan"}
          </Button>
        )}
      </div>
      <ScanStatus compact={compact} />
      {!selection?.scanner && (
        <p className="text-xs text-muted-foreground">
          No supported scanner is installed. Install ncdu 2.x, then refresh backend detection.
        </p>
      )}
      {backendError && <p className="text-xs text-destructive">{backendError}</p>}
    </div>
  );
}

function ScanStatus({ compact }: { compact: boolean }) {
  const { scan } = useApp();
  if (scan.status === "scanning")
    return (
      <div className="flex min-w-0 gap-2 text-xs text-muted-foreground">
        <span className="shrink-0">
          Scanning {plural(scan.progress.scanned, "entry", "entries")}…
        </span>
        <span className="min-w-0 truncate font-mono">{scan.progress.currentPath}</span>
      </div>
    );
  if (scan.status === "cancelled") return <p className="text-xs">Scan cancelled.</p>;
  if (scan.status === "failed") return <p className="text-xs text-destructive">{scan.message}</p>;
  if (scan.status === "done")
    return (
      <p className="min-w-0 truncate text-xs text-muted-foreground">
        <span className="font-mono">{scan.summary.rootPath}</span> ·{" "}
        {formatCount(scan.summary.entries)} entries · {scan.summary.backendId}
        {compact ? "" : ` ${scan.summary.backendVersion ?? ""}`}
      </p>
    );
  return null;
}
