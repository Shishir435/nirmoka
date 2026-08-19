import { ExternalLink, FolderOpen, Sparkles, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type { AppFootprint, CleanupPreview } from "@nirmoka/transport";

import { AppFootprintSummary } from "@/components/app-footprint-summary";
import { AppHeader } from "@/components/app-header";
import { SectionTitle } from "@/components/shared";
import { StorageComponentRow } from "@/components/storage-component-row";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { useApp } from "@/lib/app-context";
import { componentShare, reclaimableFor } from "@/lib/engine/inspector";
import { plural } from "@/lib/format";

/**
 * What one application costs, and what that is made of.
 *
 * The screen the attribution work exists for: `app_footprint` has been
 * computable since step 14 and until now the only place it appeared was a
 * number in a list. See ADR 0028 for what a footprint is and, just as
 * importantly, what it is not.
 */
export function InspectorPage({ nodeId, onBack }: { nodeId: number; onBack: () => void }) {
  const { transport, scan, features } = useApp();
  const summary = scan.status === "done" ? scan.summary : null;
  const [footprint, setFootprint] = useState<AppFootprint | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [icon, setIcon] = useState<string | null>(null);
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => {
    // Both are required to ask anything, and a missing one must not become an
    // argument: Rust reports a node id it was never given as an invalid call,
    // which is true but says nothing about what went wrong here.
    if (!summary || !Number.isInteger(nodeId)) return;
    let live = true;
    setFootprint(null);
    setError(null);
    transport.appFootprint(summary.scanId, nodeId).then(
      (value) => live && setFootprint(value),
      // An id from a previous scan resolves to nothing, or to a different
      // directory. Rust rejects the pair rather than answering about whatever
      // now sits at that index, and this is where that arrives.
      (reason: unknown) => live && setError(String(reason)),
    );
    transport.applicationIcon(summary.scanId, nodeId).then(
      (value) => live && setIcon(value),
      () => {
        // Decoration.
      },
    );
    // Whatever review is already held. Never runs one: a dry run costs minutes
    // and opening an application is not asking for it.
    transport.latestCleanupPreview().then(
      (value) => live && setPreview(value),
      () => {
        // Nothing held is the ordinary case.
      },
    );
    return () => {
      live = false;
    };
  }, [nodeId, summary, transport]);

  const home = summary?.rootPath.startsWith("/Users/")
    ? summary.rootPath.split("/").slice(0, 3).join("/")
    : "";
  const reclaimable = useMemo(
    () => (footprint ? reclaimableFor(footprint, preview, home) : []),
    [footprint, preview, home],
  );

  if (error) {
    return (
      <Card className="shadow-none">
        <CardContent className="space-y-3 p-5">
          <p className="text-sm text-destructive">{error}</p>
          <Button variant="outline" size="sm" onClick={onBack}>
            Back to the dashboard
          </Button>
        </CardContent>
      </Card>
    );
  }

  if (!footprint) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-20 w-full rounded-xl" />
        <Skeleton className="h-64 w-full rounded-xl" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <AppHeader footprint={footprint} icon={icon} />

      <Card className="shadow-none">
        <CardContent className="p-5">
          <AppFootprintSummary footprint={footprint} />
        </CardContent>
      </Card>

      <Card className="shadow-none">
        <CardContent className="p-5">
          <SectionTitle title="What's taking up space" />
          <div className="-mx-2 divide-y">
            {footprint.components.map((component) => (
              <StorageComponentRow
                key={component.label}
                component={component}
                share={componentShare(component, footprint)}
                expanded={open === component.label}
                onToggle={() => setOpen(open === component.label ? null : component.label)}
              />
            ))}
          </div>
        </CardContent>
      </Card>

      <ReclaimableForApp rows={reclaimable} name={footprint.name} hasPreview={preview !== null} />

      <Card className="shadow-none">
        <CardContent className="p-5">
          <SectionTitle title="Actions" />
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                summary && void transport.revealInFileManager(summary.scanId, footprint.nodeId)
              }
            >
              <FolderOpen /> {features?.revealLabel ?? "Reveal"}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() =>
                summary && void transport.openApplication(summary.scanId, footprint.nodeId)
              }
            >
              <ExternalLink /> Open {footprint.name}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                window.location.hash = "/storage/applications";
              }}
            >
              <Trash2 /> Uninstall…
            </Button>
          </div>
          <p className="mt-3 text-xs text-muted-foreground">
            Uninstalling is Mole&apos;s command, run against the name it publishes. Nirmoka
            assembles no paths for it — see ADR 0027.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}

/**
 * Mole's cleanup rows that fall inside this application.
 *
 * No safety badge and no rationale: the backend publishes neither, and writing
 * them here would be Nirmoka vouching for a removal it did not select. What is
 * shown is Mole's category, Mole's path, and Mole's size. See ADR 0030.
 */
function ReclaimableForApp({
  rows,
  name,
  hasPreview,
}: {
  rows: ReturnType<typeof reclaimableFor>;
  name: string;
  hasPreview: boolean;
}) {
  if (!hasPreview) {
    return (
      <div className="rounded-xl border border-dashed px-4 py-3.5 text-xs text-muted-foreground">
        No cleanup review is loaded. Running one takes a few minutes, and the dashboard offers it —
        anything it finds for {name} will appear here.
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className="rounded-xl border px-4 py-3.5 text-xs text-muted-foreground">
        Mole&apos;s current review lists nothing inside {name}&apos;s footprint.
      </div>
    );
  }

  return (
    <Card className="shadow-none">
      <CardContent className="p-5">
        <SectionTitle title="In Mole's cleanup review" />
        <div className="divide-y">
          {rows.map(({ category, item }) => (
            <div key={item.path} className="flex items-center gap-3 py-2.5">
              <Sparkles className="size-4 shrink-0 text-muted-foreground" />
              <div className="min-w-0 flex-1">
                <p className="text-sm">{category}</p>
                <p className="truncate font-mono text-xs text-muted-foreground" dir="ltr">
                  {item.path}
                </p>
              </div>
              <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
                {item.reportedSize ?? "size not reported"} ·{" "}
                {plural(Number(item.itemCount), "item", "items")}
              </span>
            </div>
          ))}
        </div>
        <p className="mt-3 text-xs text-muted-foreground">
          Mole selected these paths and applies its own protection rules. Nirmoka added none of them
          and can remove nothing Mole did not choose. Removal happens on the Clean screen.
        </p>
      </CardContent>
    </Card>
  );
}
