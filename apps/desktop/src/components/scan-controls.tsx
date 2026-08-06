import { ScanLine, Square } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { canStartScan } from "@/lib/engine/scan-machine";
import { formatCount, plural } from "@/lib/format";

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
  );
}

/**
 * Whatever the current scan has to say, on its own line under the header rule.
 * Renders nothing when there is nothing to report, so the header does not carry
 * an empty strip.
 */
export function ScanStatusStrip() {
  const { scan, selection, backendError } = useApp();
  const lines = statusLines({ scan, scanner: selection?.scanner ?? undefined, backendError });
  if (lines.length === 0) return null;

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b bg-card/40 px-8 py-1.5 text-xs max-[960px]:px-5">
      {lines.map((line) => (
        <span
          key={line.text}
          className={
            line.tone === "error"
              ? "min-w-0 truncate text-destructive"
              : "min-w-0 truncate text-muted-foreground"
          }
        >
          {line.text}
        </span>
      ))}
    </div>
  );
}

type StatusLine = { text: string; tone: "muted" | "error" };

function statusLines({
  scan,
  scanner,
  backendError,
}: {
  scan: ReturnType<typeof useApp>["scan"];
  scanner: string | undefined;
  backendError: string | null;
}): StatusLine[] {
  const lines: StatusLine[] = [];
  if (scan.status === "scanning")
    lines.push(
      { text: `Scanning ${plural(scan.progress.scanned, "entry", "entries")}…`, tone: "muted" },
      { text: scan.progress.currentPath, tone: "muted" },
    );
  if (scan.status === "cancelled") lines.push({ text: "Scan cancelled.", tone: "muted" });
  if (scan.status === "failed") lines.push({ text: scan.message, tone: "error" });
  if (scan.status === "done")
    lines.push({
      text: `${scan.summary.rootPath} · ${formatCount(scan.summary.entries)} entries · ${scan.summary.backendId}`,
      tone: "muted",
    });
  if (!scanner)
    lines.push({
      text: "No supported scanner is installed. Install ncdu 2.x, then refresh backend detection.",
      tone: "muted",
    });
  if (backendError) lines.push({ text: backendError, tone: "error" });
  return lines;
}
