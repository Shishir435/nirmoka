import { ChevronRight, Folder } from "lucide-react";

import type { CategoryConsumer, StorageCategory } from "@nirmoka/transport";

import { CATEGORY_DISPLAY } from "@/lib/category-display";
import { formatBytes } from "@/lib/format";

/**
 * What a row's number means.
 *
 * A directory's size is its size. An application's is its footprint — the
 * bundle plus everything under `~/Library` carrying its identifier — which is a
 * different quantity and much the larger one. The two sit in the same list
 * because the disk holds them both, so each row says which it is rather than
 * leaving the reader to assume they are comparable.
 */
export type ConsumerMeasure = "size" | "bundle" | "footprint";

const MEASURE_LABEL: Record<ConsumerMeasure, string> = {
  size: "on disk",
  bundle: "bundle only",
  footprint: "total footprint",
};

export function StorageConsumerRow({
  consumer,
  category,
  largestBytes,
  measure = "size",
  icon,
  onOpen,
}: {
  consumer: CategoryConsumer;
  category: StorageCategory;
  /** The biggest row in this list, so every bar shares one scale. */
  largestBytes: number;
  measure?: ConsumerMeasure;
  /** A `data:` URL from `applicationIcon`, or null. Decoration. */
  icon?: string | null;
  onOpen?: () => void;
}) {
  const display = CATEGORY_DISPLAY[category];
  const share = largestBytes === 0 ? 0 : consumer.totalBytes / largestBytes;

  return (
    <button
      type="button"
      onClick={onOpen}
      disabled={!onOpen}
      title={consumer.path}
      className="flex w-full items-center gap-3 rounded-lg px-2 py-2.5 text-left transition-colors enabled:hover:bg-accent/60 disabled:cursor-default"
    >
      {icon ? (
        <img src={icon} alt="" className="size-8 shrink-0 rounded-md" />
      ) : (
        <span
          className="grid size-8 shrink-0 place-items-center rounded-md bg-muted"
          style={{ color: display.color }}
        >
          <Folder className="size-4" />
        </span>
      )}

      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-medium">{consumer.name}</p>
        <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-muted">
          <div
            className="h-full rounded-full"
            style={{ background: display.color, width: `${Math.min(100, share * 100)}%` }}
          />
        </div>
      </div>

      <div className="shrink-0 text-right">
        <p className="text-sm font-medium tabular-nums">
          {consumer.sizeIsPartial ? "at least " : ""}
          {formatBytes(consumer.totalBytes)}
        </p>
        <p className="text-[11px] text-muted-foreground">{MEASURE_LABEL[measure]}</p>
      </div>

      {onOpen ? <ChevronRight className="size-4 shrink-0 text-muted-foreground" /> : null}
    </button>
  );
}
