import { Package } from "lucide-react";

import type { AppFootprint } from "@nirmoka/transport";

import { formatBytes } from "@/lib/format";
import { isLowerBound } from "@/lib/engine/inspector";

/**
 * The application, its footprint, and where it lives.
 *
 * The number is what the application costs — the bundle plus everything under
 * `~/Library` carrying its identifier — which is a different and much larger
 * quantity than the bundle's size on disk. See ADR 0028.
 */
export function AppHeader({ footprint, icon }: { footprint: AppFootprint; icon?: string | null }) {
  const bound = isLowerBound(footprint);

  return (
    <div className="flex items-start gap-4">
      {icon ? (
        <img src={icon} alt="" className="size-14 shrink-0 rounded-xl" />
      ) : (
        <span className="grid size-14 shrink-0 place-items-center rounded-xl bg-muted text-muted-foreground">
          <Package className="size-6" />
        </span>
      )}
      <div className="min-w-0 flex-1">
        <h1 className="truncate text-2xl font-semibold tracking-tight">{footprint.name}</h1>
        <p className="truncate font-mono text-xs text-muted-foreground" dir="ltr">
          {footprint.path}
        </p>
        {footprint.lastUsedMs !== null && (
          <p className="mt-1 text-xs text-muted-foreground">
            Last used {new Date(footprint.lastUsedMs).toLocaleDateString()}
          </p>
        )}
      </div>
      <div className="shrink-0 text-right">
        <p className="text-2xl font-semibold tabular-nums">
          {bound && <span className="text-base font-normal text-muted-foreground">at least </span>}
          {formatBytes(footprint.totalBytes)}
        </p>
        <p className="text-xs text-muted-foreground">
          {footprint.bundleId ? "Total footprint" : "Bundle only — no identifier"}
        </p>
      </div>
    </div>
  );
}
