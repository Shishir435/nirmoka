import {
  AppWindow,
  Code2,
  FileText,
  MoreHorizontal,
  Settings2,
  type LucideIcon,
} from "lucide-react";

import type { StorageCategory } from "@nirmoka/transport";

/**
 * How each category is drawn.
 *
 * Kept beside the categories rather than inside a component because three
 * things render them — the bar, the grid, and the legend — and a colour that
 * disagreed between the bar and the card it explains would read as two
 * different quantities.
 *
 * Rust decides what the categories *are* and always reports all five, in this
 * order. This file only decides what they look like.
 */
export interface CategoryDisplay {
  label: string;
  /** What the category means, for the card that has room to say so. */
  hint: string;
  icon: LucideIcon;
  /** A CSS colour, resolved from the theme tokens so dark mode follows. */
  color: string;
}

export const CATEGORY_DISPLAY: Record<StorageCategory, CategoryDisplay> = {
  apps: {
    label: "Apps",
    hint: "Applications and the data they keep for you",
    icon: AppWindow,
    color: "var(--chart-2)",
  },
  personalFiles: {
    label: "Personal Files",
    hint: "Documents, Desktop, Downloads, and your media",
    icon: FileText,
    color: "var(--chart-3)",
  },
  development: {
    label: "Development",
    hint: "Build output, toolchains, and package caches",
    icon: Code2,
    color: "var(--chart-4)",
  },
  system: {
    label: "System",
    hint: "macOS itself and what it installs",
    icon: Settings2,
    color: "var(--success)",
  },
  other: {
    label: "Other",
    hint: "Everything the rules above do not claim",
    icon: MoreHorizontal,
    color: "var(--chart-5)",
  },
};

/** Free space is not a category — nothing is using it — but it shares the bar. */
export const FREE_SPACE_COLOR = "var(--muted)";

/**
 * Space in use that this scan did not look at.
 *
 * Visibly darker than free space on purpose: these bytes are occupied, and a
 * slice that read as free would overstate the room available by however much
 * of the disk went unscanned.
 */
export const UNSCANNED_COLOR = "var(--muted-foreground)";
