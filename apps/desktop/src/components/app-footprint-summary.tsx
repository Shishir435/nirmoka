import type { AppFootprint } from "@nirmoka/transport";

import { StorageUsageBar, StorageUsageLegend } from "@/components/storage-usage-bar";
import { formatBytes } from "@/lib/format";

/**
 * The footprint as one bar, above the rows that make it up.
 *
 * Coloured by position rather than by meaning: a component is a location, and
 * there is no fixed set of them to give fixed colours to — an application with
 * no Logs directory simply has no Logs row.
 */
const COMPONENT_COLORS = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--success)",
  "var(--chart-5)",
];

export function AppFootprintSummary({
  footprint,
  compact = false,
}: {
  footprint: AppFootprint;
  compact?: boolean;
}) {
  // The guess is left out of the bar for the same reason it is left out of the
  // total: it is beside the footprint, not part of it — ADR 0028.
  const certain = footprint.components.filter((component) => component.certain);
  const slices = certain.map((component, index) => ({
    key: component.label,
    label: component.label,
    bytes: component.totalBytes,
    color: COMPONENT_COLORS[index % COMPONENT_COLORS.length]!,
  }));

  return (
    <div>
      <StorageUsageBar slices={slices} total={footprint.totalBytes} />
      {compact ? (
        <div className="mt-3 space-y-2">
          {slices.map((slice) => (
            <div key={slice.key} className="flex items-center gap-2 text-xs">
              <span className="size-2 shrink-0 rounded-full" style={{ background: slice.color }} />
              <span className="min-w-0 flex-1 truncate">{slice.label}</span>
              <span className="shrink-0 tabular-nums text-muted-foreground">
                {formatBytes(slice.bytes)}
              </span>
            </div>
          ))}
        </div>
      ) : (
        <div className="mt-4">
          <StorageUsageLegend slices={slices} />
        </div>
      )}
      {!compact && footprint.relatedBytes > 0 && (
        <p className="mt-4 text-xs text-muted-foreground">
          A further {formatBytes(footprint.relatedBytes)} sits in directories named for this
          application rather than for its identifier. It may belong to something else by the same
          vendor, so it is listed below and left out of the total.
        </p>
      )}
    </div>
  );
}
