import { HardDrive, Sparkles, Square } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { CategoryBreakdown, ScanSummary } from "@nirmoka/transport";

import { SectionTitle } from "@/components/shared";
import { StorageCategorySummary } from "@/components/storage-category-summary";
import { StorageConsumerRow } from "@/components/storage-consumer-row";
import { StorageUsageBar, StorageUsageLegend } from "@/components/storage-usage-bar";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { CATEGORY_DISPLAY, FREE_SPACE_DISPLAY } from "@/lib/category-display";
import {
  applyFootprint,
  applyIcon,
  barTotal,
  barVolume,
  unscannedBytes,
  isBundle,
  openTarget,
  rankConsumers,
  usageSlices,
  EMPTY_NARRATION,
  absorb,
  type Narration,
  type RankedConsumer,
} from "@/lib/engine/dashboard";
import { useApp } from "@/lib/app-context";
import { formatBytes, formatCount } from "@/lib/format";

/** Rows in the biggest-users list. A summary, not a browser. */
const TOP_CONSUMERS = 6;

/**
 * Application footprints fetched at once.
 *
 * Each can walk `~/Library` when the scan did not cover it, so they are not all
 * fired together: the list is readable from its first paint and the numbers
 * upgrade as they arrive. The queue is every installed application rather than
 * a chosen few — the backend caps ordinary rows and exempts bundles, because a
 * bundle's size cannot say whether its footprint belongs at the top — and it is
 * worked largest-bundle-first, so the rows most likely to move settle early.
 */
const FOOTPRINT_CONCURRENCY = 2;

/**
 * What the whole scan adds up to, and what is using it.
 *
 * Replaces the donut-and-metrics Overview. The donut described the scan; this
 * describes the disk — see ADR 0026 for why those were the same page and
 * ADR 0028 for why an application's row is not its bundle's size.
 */
export function SummarySection({
  summary,
  onOpen,
}: {
  summary: ScanSummary;
  /** `null` opens the scan root, which is how the browser addresses it. */
  onOpen?: (nodeId: number | null) => void;
}) {
  const { transport } = useApp();
  const [breakdown, setBreakdown] = useState<CategoryBreakdown | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    setBreakdown(null);
    setError(null);
    transport.categoryBreakdown(summary.scanId).then(
      (value) => live && setBreakdown(value),
      (reason: unknown) => live && setError(String(reason)),
    );
    return () => {
      live = false;
    };
  }, [summary.scanId, transport]);

  const consumers = useTopConsumers(breakdown, summary.scanId);
  const folderIcon = useFolderIcon();

  const slices = useMemo(
    () => (breakdown ? usageSlices(breakdown, CATEGORY_DISPLAY) : []),
    [breakdown],
  );

  if (error) {
    return (
      <Card className="shadow-none">
        <CardContent className="p-5 text-sm text-destructive">{error}</CardContent>
      </Card>
    );
  }

  if (!breakdown) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-36 w-full rounded-xl" />
        <Skeleton className="h-52 w-full rounded-xl" />
      </div>
    );
  }

  const volume = breakdown.volume;
  // The volume the bar could honestly be drawn against, which is not every
  // volume that was read: a scan that crossed onto another filesystem does not
  // fit inside this one's capacity.
  const framed = barVolume(breakdown);
  // What the bar does not cover, stated rather than drawn.
  const unscanned = unscannedBytes(breakdown);
  // Trimmed here rather than in the hook, so a footprint that arrives late can
  // still promote its row into view.
  const visible = consumers.slice(0, TOP_CONSUMERS);
  const largest = visible[0]?.consumer.totalBytes ?? 0;

  return (
    <div className="space-y-6">
      <Card className="shadow-none">
        <CardContent className="p-5">
          <div className="flex items-start gap-4">
            <span className="grid size-11 shrink-0 place-items-center rounded-xl bg-muted text-muted-foreground">
              <HardDrive className="size-5" />
            </span>
            <div className="min-w-0 flex-1">
              <h2 className="truncate text-lg font-semibold tracking-tight">
                {volume?.name ?? breakdown.rootPath}
              </h2>
              <p className="truncate text-xs text-muted-foreground">
                {volume ? volume.mountPoint : "Volume capacity unavailable"} ·{" "}
                {formatCount(summary.entries)} entries scanned
              </p>
            </div>
            {volume ? (
              <div className="flex shrink-0 gap-6 text-right">
                <div>
                  <p className="text-lg font-semibold tabular-nums">
                    {formatBytes(volume.usedBytes)}
                  </p>
                  <p className="text-xs text-muted-foreground">Used</p>
                </div>
                <div>
                  <p className="text-lg font-semibold tabular-nums">
                    {formatBytes(volume.freeBytes)}
                  </p>
                  <p className="text-xs text-muted-foreground">Free</p>
                </div>
              </div>
            ) : null}
          </div>

          <StorageUsageBar slices={slices} total={barTotal(breakdown)} className="mt-5" />
          <div className="mt-4">
            <StorageUsageLegend slices={slices} />
          </div>

          {/* The scan and the volume are different numbers whenever the scan was
              not the whole disk, and saying so is cheaper than a user working
              out why the bar does not fill. A scan larger than the volume is the
              other direction of the same problem: it crossed onto another
              filesystem, the bar is the scan, and that is worth stating rather
              than leaving as a chart that quietly changed what it measures. */}
          {volume && !framed ? (
            <p className="mt-4 text-xs text-muted-foreground">
              This scan measured {formatBytes(breakdown.scannedBytes)}, more than {volume.name}{" "}
              holds, so it reached onto another volume. The bar divides the scan.
            </p>
          ) : unscanned > 0 ? (
            <p className="mt-4 text-xs text-muted-foreground">
              The bar divides the {formatBytes(breakdown.scannedBytes)} this scan covered.{" "}
              {formatBytes(unscanned)} more is in use on this volume and was not looked at.
            </p>
          ) : null}
        </CardContent>
      </Card>

      {/* Two columns, as the design draws it: the grid of categories beside the
          list of what is actually large. They answer different questions — what
          kind of thing, and which thing — and stacking them made the second one
          a scroll away. */}
      <div className="grid grid-cols-[minmax(0,1.15fr)_minmax(0,1fr)] gap-6 max-[1100px]:grid-cols-1">
        <div>
          <SectionTitle title="What's using your space" />
          <div className="grid grid-cols-3 gap-3 max-[520px]:grid-cols-2">
            {breakdown.categories.map((category) => (
              <StorageCategorySummary
                key={category.category}
                summary={category}
                scannedBytes={breakdown.scannedBytes}
              />
            ))}
            {/* Free space is not a category — nothing is using it — but "how
                much room is left" is the question the window is opened with,
                and the design gives it the sixth tile rather than a footnote. */}
            {framed ? (
              <StorageCategorySummary
                display={FREE_SPACE_DISPLAY}
                summary={{
                  category: "other",
                  totalBytes: framed.freeBytes,
                  share: framed.totalBytes === 0 ? 0 : framed.freeBytes / framed.totalBytes,
                  consumers: [],
                }}
                scannedBytes={framed.totalBytes}
                shareOf="volume"
              />
            ) : null}
          </div>
        </div>

        <Card className="shadow-none">
          <CardContent className="p-5">
            <SectionTitle title="Biggest space users" />
            {visible.length === 0 ? (
              <p className="py-6 text-center text-sm text-muted-foreground">
                This scan found nothing large enough to list.
              </p>
            ) : (
              <div className="-mx-2 divide-y">
                {visible.map((entry) => (
                  <StorageConsumerRow
                    key={`${entry.category}:${entry.consumer.id}`}
                    consumer={entry.consumer}
                    category={entry.category}
                    largestBytes={largest}
                    measure={entry.measure}
                    icon={entry.icon}
                    folderIcon={folderIcon}
                    onOpen={onOpen ? () => onOpen(openTarget(entry)) : undefined}
                  />
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <ReclaimableBanner />

      {summary.readErrors > 0 || summary.excluded > 0 ? (
        <p className="rounded-lg border border-warning/30 bg-warning/10 p-3 text-xs text-warning-foreground">
          These totals are a lower bound: {formatCount(summary.readErrors)} unreadable and{" "}
          {formatCount(summary.excluded)} excluded entries.
        </p>
      ) : null}
    </div>
  );
}

/**
 * The desktop's folder icon, fetched once for the whole list.
 *
 * It does not depend on which directory it stands for, so asking per row would
 * be the same file read six times.
 */
function useFolderIcon() {
  const { transport } = useApp();
  const [icon, setIcon] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    transport.folderIcon().then(
      (value) => live && setIcon(value),
      () => {
        // Decoration. The drawn fallback stands.
      },
    );
    return () => {
      live = false;
    };
  }, [transport]);

  return icon;
}

/**
 * The biggest entries across every category, enriched where it is worth it.
 *
 * Applications are upgraded twice: an icon, which is decoration, and a
 * footprint, which is the honest number for an application and is not its
 * bundle's size. Both arrive late and neither blocks the list.
 */
function useTopConsumers(breakdown: CategoryBreakdown | null, scanId: number) {
  const { transport } = useApp();
  const [rows, setRows] = useState<RankedConsumer[]>([]);

  // Every candidate, not the visible six: a footprint can lift an application
  // from tenth place to first, and it cannot do that if it was already dropped.
  const base = useMemo(() => rankConsumers(breakdown), [breakdown]);

  useEffect(() => {
    setRows(base);
    const bundles = base.filter((row) => isBundle(row.consumer));
    if (bundles.length === 0) return;

    let live = true;

    // Icons are small, independent, and never reorder anything.
    for (const row of bundles) {
      transport
        .applicationIcon(scanId, row.consumer.id)
        .then((icon) => live && setRows((current) => applyIcon(current, row.consumer.id, icon)))
        .catch(() => {
          // Decoration. A bundle with no readable icon keeps its fallback.
        });
    }

    // A footprint can walk the disk, so they go a couple at a time and the list
    // stays readable from its first paint.
    const queue = [...bundles];
    const worker = async (): Promise<void> => {
      // `live` is cleared by the cleanup below rather than by this loop, so the
      // check is the first statement rather than the loop condition.
      for (;;) {
        if (!live) return;
        const row = queue.shift();
        if (!row) return;
        try {
          const footprint = await transport.appFootprint(scanId, row.consumer.id);
          if (!live) return;
          setRows((current) => applyFootprint(current, row.consumer.id, footprint.totalBytes));
        } catch {
          // The row keeps the bundle size it has, still labelled as one.
        }
      }
    };
    void Promise.all(Array.from({ length: Math.min(FOOTPRINT_CONCURRENCY, queue.length) }, worker));

    return () => {
      live = false;
    };
  }, [base, scanId, transport]);

  return rows;
}

/**
 * What a cleanup would free, in Mole's own words.
 *
 * Never an estimate of Nirmoka's — see ADR 0030. And never run on arrival: a
 * dry run is a subprocess, and a page that starts one every time it is opened
 * is the mistake ADR 0026 removed from System Status. The number appears when
 * the user asks for it.
 */
function ReclaimableBanner() {
  const { transport } = useApp();
  const [state, setState] = useState<
    { kind: "idle" } | { kind: "checking" } | { kind: "known"; total: string } | { kind: "none" }
  >({ kind: "idle" });
  const [elapsed, setElapsed] = useState(0);
  const [narration, setNarration] = useState<Narration>(EMPTY_NARRATION);

  // Measured at 2m26s against Mole 1.48.1 on a full disk. A spinner with no
  // clock reads as a hang at that length, and the button that would stop it has
  // to be reachable rather than disabled.
  useEffect(() => {
    if (state.kind !== "checking") return;
    setElapsed(0);
    const tick = setInterval(() => setElapsed((seconds) => seconds + 1), 1000);
    return () => clearInterval(tick);
  }, [state.kind]);

  // Subscribed before the preview is asked for: registering is a round trip
  // into Rust, and a run that started first would narrate into nothing.
  const check = async () => {
    setNarration(EMPTY_NARRATION);
    setState({ kind: "checking" });
    const unsubscribe = await transport.onCleanupProgress((progress) =>
      setNarration((current) => absorb(current, progress)),
    );
    try {
      const preview = await transport.cleanupPreview();
      setState(
        preview.potentialCleanup
          ? { kind: "known", total: preview.potentialCleanup }
          : { kind: "none" },
      );
    } catch {
      // Includes the cancellation below: a stopped run has no figure, and the
      // banner goes back to offering one rather than reporting a failure.
      setState({ kind: "idle" });
    } finally {
      unsubscribe();
    }
  };

  const stop = () => {
    void transport.cancelCleanupPreview();
    setState({ kind: "idle" });
  };

  if (state.kind === "none") return null;

  return (
    <div className="rounded-xl border border-success/20 bg-success/10 px-4 py-3.5">
      <div className="flex items-center gap-3">
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-background text-success shadow-xs">
          <Sparkles className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          {state.kind === "known" ? (
            <>
              <p className="text-sm font-medium">{state.total} can be reclaimed</p>
              <p className="text-xs text-muted-foreground">
                Mole&apos;s own figure, from its dry run. Review it before anything is removed.
              </p>
            </>
          ) : state.kind === "checking" ? (
            <>
              <p className="text-sm font-medium">
                Checking what can be reclaimed · {formatElapsed(elapsed)}
              </p>
              <p className="truncate text-xs text-muted-foreground">
                {narration.category
                  ? `${narration.category} · group ${narration.categoriesSeen}${
                      narration.lastTotal ? ` · came to ${narration.lastTotal}` : ""
                    }`
                  : "Mole is walking the disk. Nothing is being removed."}
              </p>
            </>
          ) : (
            <>
              <p className="text-sm font-medium">Check what can be reclaimed</p>
              <p className="text-xs text-muted-foreground">
                Runs Mole&apos;s dry run, which takes a few minutes. Nothing is removed, and the
                figure is Mole&apos;s rather than an estimate of ours.
              </p>
            </>
          )}
        </div>
        {state.kind === "checking" ? (
          <Button variant="outline" size="sm" onClick={stop}>
            <Square /> Stop
          </Button>
        ) : (
          <Button
            variant={state.kind === "known" ? "default" : "outline"}
            size="sm"
            onClick={
              state.kind === "known"
                ? () => {
                    window.location.hash = "/clean";
                  }
                : () => void check()
            }
          >
            {state.kind === "known" ? "Review" : "Check"}
          </Button>
        )}
      </div>

      {/* The line the backend is on. Fixed height and truncated: Mole reports
          several a second and names of wildly different lengths, and letting one
          wrap would move every pixel below it. */}
      {state.kind === "checking" && narration.item ? (
        <p className="mt-2.5 h-4 truncate pl-12 font-mono text-[11px] text-muted-foreground">
          {narration.item}
        </p>
      ) : null}
    </div>
  );
}

/** `m:ss`, so a wait of minutes reads as one rather than as a growing integer. */
function formatElapsed(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
