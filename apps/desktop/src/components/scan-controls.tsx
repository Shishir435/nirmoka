import { ScanLine, Square } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { canStartScan, scanStatusLine } from "@/lib/engine/scan-machine";
import { formatCount } from "@/lib/format";

/**
 * The scan bar, in the shell header. Rust holds exactly one scan at a time, so a
 * second copy of these controls implied a second scan — see ADR 0026.
 *
 * Deliberately one row and nothing else. Anything a scan has to say arrives in
 * `ScanStatusStrip` below the header rule, so this row keeps a fixed height and
 * stays level with the brand block across the sidebar border. A status line
 * inside it would move the rule every time a scan started.
 */
export function ScanBar() {
  const { selection, listenersReady, scan, startScan, cancelScan } = useApp();
  const [path, setPath] = useState("~");
  const canScan = canStartScan({ scanner: selection?.scanner, listenersReady, state: scan });

  // Only when there is something to stop or something to re-run. Before a scan —
  // and after one that was cancelled or failed — the start screen owns choosing a
  // directory, and this row was an empty text field the width of the window
  // beside a second primary button doing the same job as the one below it: the
  // duplication ADR 0026 removed, reintroduced one level down. What went wrong is
  // reported by `ScanStatusStrip`, which needs no control of its own.
  if (scan.status !== "scanning" && scan.status !== "done") return null;

  return (
    <div className="flex min-w-0 flex-1 items-center gap-2">
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
        // Bounded rather than flex-1. A path is short and a scan root is
        // shorter; stretching the field to the full window width made an empty
        // input the largest object on the screen.
        className="h-9 w-full max-w-96 min-w-48 rounded-md border bg-background px-3 font-mono text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/20 disabled:opacity-50"
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
  );
}

/**
 * Whatever the current scan has to say, on one line under the header rule.
 *
 * Fixed height, one line, and the path truncated rather than wrapped. A scanner
 * walking a deep tree reports paths of wildly different lengths several times a
 * second; letting one of them wrap moved every pixel of the page below it. The
 * count is tabular and given a floor width for the same reason horizontally, so
 * 6,25,001 growing to 11,00,001 does not drag the path along with it.
 */
export function ScanStatusStrip() {
  const { scan, selection, backendError } = useApp();
  const line = scanStatusLine({
    state: scan,
    scanner: selection?.scanner ?? undefined,
    backendError,
    formatCount,
  });
  if (!line) return null;

  return (
    <div className="flex h-8 shrink-0 items-center gap-3 overflow-hidden border-b bg-card/40 px-8 text-xs max-[960px]:px-5">
      <span
        className={`shrink-0 tabular-nums ${
          line.tone === "error" ? "text-destructive" : "text-muted-foreground"
        } ${scan.status === "scanning" ? "min-w-52" : ""}`}
      >
        {line.label}
      </span>
      {line.detail && (
        <span
          className={`min-w-0 truncate font-mono ${
            line.tone === "error" ? "text-destructive" : "text-muted-foreground"
          }`}
          // The end of a path identifies it; the start is shared by everything
          // under the scan root. Keeping direction explicit means the ellipsis
          // does not move to the other end under a right-to-left system locale.
          dir="ltr"
        >
          {line.detail}
        </span>
      )}
    </div>
  );
}
