import {
  Box,
  Brush,
  Code2,
  Database,
  Folder,
  LayoutDashboard,
  Search,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { useState } from "react";

import { PageHeader } from "@/components/shared";
import { Input } from "@/components/ui/input";

const helpCards = [
  {
    title: "Getting Started",
    text: "Install ncdu 2.x, type a directory in the bar at the top, and press Scan. Scanning changes nothing.",
    Icon: Folder,
  },
  {
    title: "Three Places",
    text: "Storage is the scan and every view of it. Clean is Mole's cleanup. Activity is everything that was removed.",
    Icon: LayoutDashboard,
  },
  {
    title: "Moving to the Trash",
    text: "Select a row in Storage and press ⌘⌫ or the Trash button. The undo is Put Back in the Finder; permanent deletion is not offered.",
    Icon: Trash2,
  },
  {
    title: "Cleaning Safety",
    text: "Clean shows Mole's own preview, and running it calls Mole's command with no paths from this window. Mole re-discovers candidates as it runs.",
    Icon: Brush,
  },
  {
    title: "Permissions",
    text: "Unreadable entries are marked and totals are reported as lower bounds.",
    Icon: Database,
  },
  {
    title: "Backend Roles",
    text: "ncdu scans; Mole cleanup capabilities do not make Mole a scanner.",
    Icon: Box,
  },
  {
    title: "Architecture",
    text: "The full tree stays in Rust and the UI requests visible windows.",
    Icon: Code2,
  },
  {
    title: "Privacy",
    text: "Scans and the operation journal stay on this Mac.",
    Icon: ShieldCheck,
  },
];

export function HelpPage() {
  const [query, setQuery] = useState("");
  const visibleCards = helpCards.filter((card) =>
    `${card.title} ${card.text}`.toLowerCase().includes(query.toLowerCase()),
  );
  return (
    <div className="space-y-6">
      <PageHeader
        title="Help"
        subtitle="Current beta behavior"
        action={
          <div className="relative w-64">
            <Search className="absolute left-3 top-2.5 size-4 text-muted-foreground" />
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              className="pl-9"
              placeholder="Search help…"
            />
          </div>
        }
      />
      <div className="grid grid-cols-3 gap-3 max-[1050px]:grid-cols-2">
        {visibleCards.map(({ title, text, Icon }) => (
          <div key={title} className="rounded-xl border bg-card p-5 text-left">
            <Icon className="size-5 text-muted-foreground" />
            <p className="mt-5 text-sm font-medium">{title}</p>
            <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{text}</p>
          </div>
        ))}
      </div>
      <p className="rounded-xl border bg-muted/30 p-4 text-xs text-muted-foreground">
        Diagnostic report export is not implemented in this beta. Nirmoka does not claim that a
        report was saved.
      </p>
    </div>
  );
}
