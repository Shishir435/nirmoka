import { formatBytes } from "@/lib/format";
import { cn } from "@/lib/utils";

export interface UsageSlice {
  key: string;
  label: string;
  bytes: number;
  color: string;
}

/**
 * One horizontal bar for the whole volume.
 *
 * This replaces the donut the Overview page used to show. The donut answered
 * "what did the scan see", which is a question about Nirmoka; a bar that runs
 * to the volume's capacity answers "what is on this disk", which is the one the
 * user opened the window with. Free space is a slice of it for the same reason:
 * a used-only chart cannot show how much room is left, which is the whole
 * point of looking.
 *
 * Slices below a pixel or so are dropped rather than rendered as a hairline —
 * a sliver too small to see but wide enough to catch a tooltip is a target
 * nobody can hit. What survives is then measured against what survived, not
 * against the volume: the track behind the bar is the same colour as free
 * space, so a dropped sliver of occupied disk would show through as room to
 * spare. The distortion is under half a percent, which is the threshold; the
 * bytes are still named in the legend and the label.
 */
export function StorageUsageBar({
  slices,
  total,
  className,
}: {
  slices: UsageSlice[];
  total: number;
  className?: string;
}) {
  const visible = slices.filter((slice) => total > 0 && slice.bytes / total >= 0.005);
  const shown = visible.reduce((sum, slice) => sum + slice.bytes, 0);

  return (
    <div
      className={cn("flex h-3 w-full overflow-hidden rounded-full bg-muted", className)}
      role="img"
      aria-label={slices.map((slice) => `${slice.label} ${formatBytes(slice.bytes)}`).join(", ")}
    >
      {visible.map((slice) => (
        <div
          key={slice.key}
          title={`${slice.label} — ${formatBytes(slice.bytes)}`}
          style={{
            background: slice.color,
            // Percentages rather than flex-grow: a slice's width is its share
            // of the slices drawn, and flex would redistribute the rounding.
            width: `${(slice.bytes / shown) * 100}%`,
          }}
        />
      ))}
    </div>
  );
}

/** The key beneath the bar: a dot, a name, and the number it stands for. */
export function StorageUsageLegend({ slices }: { slices: UsageSlice[] }) {
  return (
    <div className="grid grid-cols-6 gap-3 max-[900px]:grid-cols-3">
      {slices.map((slice) => (
        <div key={slice.key} className="min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="size-2 shrink-0 rounded-full" style={{ background: slice.color }} />
            <span className="truncate text-xs text-muted-foreground">{slice.label}</span>
          </div>
          <p className="mt-1 pl-3.5 text-sm font-medium tabular-nums">{formatBytes(slice.bytes)}</p>
        </div>
      ))}
    </div>
  );
}
