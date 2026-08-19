import { ScanLine, Square } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useApp } from "@/lib/app-context";
import { canStartScan, scanStatusLine } from "@/lib/engine/scan-machine";
import { START_TARGETS } from "@/lib/engine/start-targets";
import { formatCount } from "@/lib/format";

/**
 * Scanning, as one control in the header.
 *
 * This was a path field and a button occupying the full width of the window,
 * which made an empty text input the largest object on screen and asked the
 * user to type a directory before anything had been shown to them. A scan has
 * one question — which directory — and it has four common answers, so the
 * control is a button and the answers are behind it. See ADR 0031.
 *
 * The label follows the state rather than the route: a window with a scan in it
 * offers to run another, and the design says "Scan Again" for that.
 */
export function ScanControl() {
  const { selection, listenersReady, scan, startScan, cancelScan } = useApp();
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState("~");
  const canScan = canStartScan({ scanner: selection?.scanner, listenersReady, state: scan });

  if (scan.status === "scanning") {
    return (
      <Button variant="destructive" size="sm" onClick={() => void cancelScan()}>
        <Square /> Stop
      </Button>
    );
  }

  const begin = (path: string) => {
    if (!canScan || !path.trim()) return;
    setOpen(false);
    void startScan(path);
  };

  return (
    <>
      <Button size="sm" disabled={!canScan} onClick={() => setOpen(true)}>
        <ScanLine /> {scan.status === "done" ? "Scan Again" : "Scan"}
      </Button>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>What should Nirmoka look at?</DialogTitle>
            <DialogDescription>
              Scanning only reads. Nothing is removed without a separate confirmation.
            </DialogDescription>
          </DialogHeader>

          <div className="grid grid-cols-2 gap-2">
            {START_TARGETS.map((target) => (
              <button
                key={target.id}
                type="button"
                onClick={() => begin(target.path)}
                className="rounded-xl border p-3 text-left transition-colors hover:border-primary/40 hover:bg-accent/60"
              >
                <p className="text-sm font-medium">{target.label}</p>
                <p className="mt-0.5 text-xs text-muted-foreground">{target.hint}</p>
              </button>
            ))}
          </div>

          <div>
            <label htmlFor="scan-path" className="text-xs font-medium text-muted-foreground">
              Or another folder
            </label>
            <div className="mt-1.5 flex gap-2">
              <input
                id="scan-path"
                value={custom}
                onChange={(event) => setCustom(event.target.value)}
                onKeyDown={(event) => event.key === "Enter" && begin(custom)}
                placeholder="~/Projects"
                spellCheck={false}
                className="h-9 min-w-0 flex-1 rounded-md border bg-background px-3 font-mono text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/20"
              />
              <Button onClick={() => begin(custom)} disabled={!custom.trim()}>
                Scan
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
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
