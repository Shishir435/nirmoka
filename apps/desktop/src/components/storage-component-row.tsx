import { ChevronRight } from "lucide-react";

import type { StorageComponent } from "@nirmoka/transport";

import { formatBytes, plural } from "@/lib/format";
import { cn } from "@/lib/utils";

/**
 * One component of an application's footprint.
 *
 * The label names a location macOS defines — Application, Containers, Caches —
 * never a concept the application keeps there. `Docker.raw` is one path at its
 * real size, and what is inside it is Docker's vocabulary for a file this
 * program can only see the outside of. See ADR 0028.
 */
export function StorageComponentRow({
  component,
  share,
  expanded,
  onToggle,
}: {
  component: StorageComponent;
  /** Of the footprint, 0..1. Zero for the component that is a guess. */
  share: number;
  expanded: boolean;
  onToggle: () => void;
}) {
  const percent = Math.round(share * 100);

  return (
    <div className={cn(!component.certain && "bg-muted/40")}>
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={expanded}
        className="flex w-full items-center gap-3 px-2 py-3 text-left transition-colors hover:bg-accent/50"
      >
        <ChevronRight
          className={cn(
            "size-4 shrink-0 text-muted-foreground transition-transform",
            expanded && "rotate-90",
          )}
        />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium">
            {component.label}
            {!component.certain && (
              <span className="ml-2 rounded bg-muted px-1.5 py-0.5 text-[11px] font-normal text-muted-foreground">
                matched by name
              </span>
            )}
          </p>
          <p className="text-xs text-muted-foreground">
            {plural(component.paths.length, "location", "locations")}
            {!component.complete && " · part of this could not be read"}
          </p>
        </div>
        <span className="shrink-0 text-sm font-medium tabular-nums">
          {component.complete ? "" : "at least "}
          {formatBytes(component.totalBytes)}
        </span>
        {/* The guess has no percentage because it is not part of the total it
            would be a percentage of — ADR 0028. */}
        <span className="w-10 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
          {component.certain ? `${percent}%` : "—"}
        </span>
      </button>

      {expanded && (
        <ul className="space-y-1 pb-3 pl-9 pr-2">
          {component.paths.map((path) => (
            <li key={path.path} className="flex items-baseline gap-3 text-xs">
              <span className="min-w-0 flex-1 truncate font-mono text-muted-foreground" dir="ltr">
                {path.path}
              </span>
              <span className="shrink-0 tabular-nums text-muted-foreground">
                {path.totalBytes === null
                  ? "size unavailable"
                  : `${path.complete ? "" : "at least "}${formatBytes(path.totalBytes)}`}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
