import { useEffect, useState } from "react";
import { ArrowRight, HardDrive } from "lucide-react";

import type { VolumeInfo } from "@nirmoka/transport";

import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { isNearlyFull, START_TARGETS, usedFraction } from "@/lib/engine/start-targets";
import { canStartScan } from "@/lib/engine/scan-machine";
import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";

/**
 * The window before anything has been scanned.
 *
 * One object, centred, holding the two things this screen knows: how full the
 * disk is, and where a scan could start. Capacity needs no backend — it is a
 * `df` call — so the first screen answers the question the app was opened to ask
 * instead of rendering a dashed box that says to type a path.
 *
 * Deliberately not a stack of full-width rows with a full-width primary button.
 * That is a web form, and it read as one: the button was the largest element in
 * the window and duplicated the scan control in the header.
 */
export function StartScreen() {
  const { transport, selection, listenersReady, scan, startScan } = useApp();
  const [volume, setVolume] = useState<VolumeInfo | null>(null);
  const [volumeError, setVolumeError] = useState<string | null>(null);
  const [custom, setCustom] = useState<string | null>(null);
  const canScan = canStartScan({ scanner: selection?.scanner, listenersReady, state: scan });

  useEffect(() => {
    let live = true;
    // The home directory rather than `/`: the sealed system volume reports its
    // own tiny usage, and the data volume is the one the user can act on.
    transport.volumeInfo("~").then(
      (value) => live && setVolume(value),
      // A window that cannot read capacity can still scan, so this degrades to
      // the targets below rather than blocking the screen.
      (error: unknown) => live && setVolumeError(String(error)),
    );
    return () => {
      live = false;
    };
  }, [transport]);

  return (
    // Centred in whatever height is left, so the card is placed rather than
    // sitting at the top of a column of empty space.
    <div className="grid min-h-full place-items-center py-6">
      <section className="w-full max-w-lg rounded-2xl border bg-card shadow-xs">
        {volume ? <Capacity volume={volume} /> : <CapacityUnavailable reason={volumeError} />}

        <div className="border-t p-5">
          <p className="text-muted-foreground mb-3 text-xs font-medium">Scan</p>

          {/* Chips rather than cards. Four scan roots are a choice of one thing,
              and each was a bordered box with a heading, a description, and a
              right-aligned path — three lines of furniture around one verb. */}
          <div className="flex flex-wrap gap-2">
            {START_TARGETS.map((target, index) => (
              <Button
                key={target.id}
                // Home is the default, so it carries the emphasis and the rest
                // are alternatives to it.
                variant={index === 0 ? "default" : "outline"}
                size="sm"
                disabled={!canScan}
                title={target.path}
                onClick={() => void startScan(target.path)}
              >
                {target.label}
              </Button>
            ))}
            <Button
              variant="ghost"
              size="sm"
              disabled={!canScan}
              onClick={() => setCustom((open) => (open === null ? "" : null))}
              aria-expanded={custom !== null}
            >
              Other folder…
            </Button>
          </div>

          {custom !== null && (
            <form
              className="mt-3 flex gap-2"
              onSubmit={(event) => {
                event.preventDefault();
                if (canScan && custom.trim()) void startScan(custom);
              }}
            >
              <input
                // The field appeared because the user asked for it, so it takes
                // the caret rather than making them click the thing they just
                // revealed. Not a page-load autofocus, which is what the rule
                // guards against.
                // oxlint-disable-next-line jsx-a11y/no-autofocus -- revealed on demand, not on load
                autoFocus
                aria-label="Directory to scan"
                value={custom}
                onChange={(event) => setCustom(event.target.value)}
                placeholder="~/Projects"
                spellCheck={false}
                className="bg-background focus-visible:ring-ring/20 h-8 min-w-0 flex-1 rounded-md border px-2.5 font-mono text-xs outline-none focus-visible:ring-3"
              />
              <Button type="submit" size="sm" disabled={!canScan || !custom.trim()}>
                <ArrowRight />
                Scan
              </Button>
            </form>
          )}

          <p className="text-muted-foreground mt-3 text-xs">Scanning only reads.</p>
        </div>
      </section>
    </div>
  );
}

/**
 * The volume, its capacity, and how much of it is gone.
 *
 * One bar, drawn used-over-total, with free space stated as its own number.
 * `used + free` does not equal `total` on macOS — the rest is reserved or
 * purgeable — so two adjacent segments would leave an unexplained gap.
 */
function Capacity({ volume }: { volume: VolumeInfo }) {
  const fraction = usedFraction(volume);
  const nearlyFull = isNearlyFull(volume);

  return (
    <div className="p-5">
      <div className="flex items-center gap-3">
        <div className="bg-muted text-muted-foreground grid size-9 shrink-0 place-items-center rounded-lg">
          <HardDrive className="size-4.5" />
        </div>
        <div className="min-w-0 flex-1">
          <h1 className="truncate text-[15px] font-semibold tracking-[-0.01em]">{volume.name}</h1>
          <p className="text-muted-foreground truncate font-mono text-[11px]">
            {volume.mountPoint}
          </p>
        </div>
        <p className="text-muted-foreground shrink-0 text-right text-xs tabular-nums">
          {formatBytes(volume.totalBytes)}
        </p>
      </div>

      <div
        className="bg-muted mt-4 h-2 overflow-hidden rounded-full"
        role="img"
        aria-label={`${formatBytes(volume.usedBytes)} of ${formatBytes(volume.totalBytes)} used, ${formatBytes(volume.freeBytes)} free`}
      >
        <div
          className={cn("h-full rounded-full", nearlyFull ? "bg-warning" : "bg-primary")}
          style={{ width: `${(fraction * 100).toFixed(1)}%` }}
        />
      </div>

      <div className="mt-2 flex items-baseline justify-between gap-4 text-xs">
        <span className="tabular-nums">
          <strong className="text-sm font-semibold">{formatBytes(volume.usedBytes)}</strong>
          <span className="text-muted-foreground"> used</span>
        </span>
        <span className="text-muted-foreground tabular-nums">
          {formatBytes(volume.freeBytes)} free
        </span>
      </div>

      {nearlyFull && (
        <p className="border-warning/30 bg-warning/10 text-warning-foreground mt-3 rounded-lg border px-3 py-2 text-xs">
          This volume is nearly full. A scan will show what is holding the space.
        </p>
      )}
    </div>
  );
}

/**
 * Capacity could not be read. The reason is shown rather than a zeroed bar,
 * because a bar at 0% is a claim about the disk and this is a claim about
 * Nirmoka. Off macOS this is the expected path — see `volume.rs`.
 */
function CapacityUnavailable({ reason }: { reason: string | null }) {
  return (
    <div className="flex items-center gap-3 p-5">
      <div className="bg-muted text-muted-foreground grid size-9 shrink-0 place-items-center rounded-lg">
        <HardDrive className="size-4.5" />
      </div>
      <div className="min-w-0">
        <h1 className="text-[15px] font-semibold tracking-[-0.01em]">Ready to scan</h1>
        <p className="text-muted-foreground truncate text-xs">
          {reason ? `Volume capacity unavailable: ${reason}` : "Reading volume capacity…"}
        </p>
      </div>
    </div>
  );
}
