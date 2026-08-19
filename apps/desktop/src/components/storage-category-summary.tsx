import type { CategorySummary } from "@nirmoka/transport";

import { CATEGORY_DISPLAY, type CategoryDisplay } from "@/lib/category-display";
import { formatBytes } from "@/lib/format";
import { Card, CardContent } from "@/components/ui/card";

/**
 * One category, as a card in the grid.
 *
 * The percentage is of the scanned size rather than of the volume, and says
 * which, because the two differ whenever the scan was not the whole disk — a
 * scan of `~/Downloads` is 100% personal files and a rounding error of the
 * volume. Free space is the one tile measured against the volume, since that is
 * the only thing it can be a share of.
 */
export function StorageCategorySummary({
  summary,
  scannedBytes,
  display: override,
  shareOf = "scan",
  onOpen,
}: {
  summary: CategorySummary;
  scannedBytes: number;
  /** For the one tile that is not a category: free space. */
  display?: CategoryDisplay;
  shareOf?: "scan" | "volume";
  onOpen?: () => void;
}) {
  const display = override ?? CATEGORY_DISPLAY[summary.category];
  const Icon = display.icon;
  const percent = scannedBytes === 0 ? 0 : Math.round(summary.share * 100);
  const interactive = Boolean(onOpen) && summary.totalBytes > 0;

  const body = (
    <CardContent className="p-4">
      <div className="flex items-center gap-2.5">
        <span
          className="grid size-8 shrink-0 place-items-center rounded-lg"
          style={{ background: display.color, color: "var(--background)" }}
        >
          <Icon className="size-4" />
        </span>
        <span className="min-w-0 flex-1 truncate text-sm font-medium">{display.label}</span>
      </div>

      <p className="mt-3 text-xl font-semibold tracking-tight tabular-nums">
        {formatBytes(summary.totalBytes)}
      </p>
      <p className="mt-0.5 text-xs text-muted-foreground">
        {percent}% of {shareOf === "volume" ? "this volume" : "what was scanned"}
      </p>

      <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-muted">
        <div
          className="h-full rounded-full"
          style={{
            background: display.color,
            width: `${Math.min(100, Math.max(0, summary.share * 100))}%`,
          }}
        />
      </div>
    </CardContent>
  );

  if (!interactive) {
    return (
      <Card className="shadow-none" title={display.hint}>
        {body}
      </Card>
    );
  }

  return (
    <Card
      className="cursor-pointer shadow-none transition-colors hover:border-primary/40"
      title={display.hint}
    >
      <button type="button" onClick={onOpen} className="w-full text-left">
        {body}
      </button>
    </Card>
  );
}
