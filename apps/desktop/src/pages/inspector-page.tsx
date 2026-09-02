import { ExternalLink, FolderOpen, Package, Sparkles, Trash2 } from "lucide-react";
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
import { formatBytes, plural } from "@/lib/format";

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
    <div className="grid grid-cols-[250px_minmax(0,1fr)] gap-6 max-[900px]:grid-cols-1">
      <aside className="space-y-3">
        <Card className="shadow-none">
          <CardContent className="p-5 text-center">
            {icon ? (
              <img src={icon} alt="" className="mx-auto size-14 rounded-xl" />
            ) : (
              <span className="mx-auto grid size-14 place-items-center rounded-xl bg-muted text-muted-foreground">
                <Package className="size-6" />
              </span>
            )}
            <h1 className="mt-3 truncate text-lg font-semibold">{footprint.name}</h1>
            <p className="mt-1 text-2xl font-semibold tracking-tight tabular-nums">
              {formatBytes(footprint.totalBytes)}
            </p>
            <p className="text-xs text-muted-foreground">
              {footprint.bundleId ? "Total footprint" : "Application bundle"}
            </p>
          </CardContent>
        </Card>

        <Card className="shadow-none">
          <CardContent className="p-4">
            <SectionTitle title="Overview" />
            <AppFootprintSummary footprint={footprint} compact />
          </CardContent>
        </Card>

        {reclaimable.length > 0 && (
          <Card className="border-success/20 bg-success/6 shadow-none">
            <CardContent className="p-4">
              <p className="text-xs font-medium">In Mole's cleanup review</p>
              <p className="mt-1 text-xl font-semibold text-success tabular-nums">
                {reclaimable.length} {plural(reclaimable.length, "item", "items")}
              </p>
              <Button
                size="sm"
                className="mt-3 w-full bg-success hover:bg-success/90"
                onClick={() => {
                  window.location.hash = "/clean";
                }}
              >
                Review cleanup
              </Button>
            </CardContent>
          </Card>
        )}

        <Card className="shadow-none">
          <CardContent className="p-3">
            <p className="px-2 pb-2 text-xs font-medium">Actions</p>
            <InspectorAction
              icon={<FolderOpen />}
              label={features?.revealLabel ?? "Reveal in Finder"}
              onClick={() =>
                summary && void transport.revealInFileManager(summary.scanId, footprint.nodeId)
              }
            />
            <InspectorAction
              icon={<ExternalLink />}
              label={`Open ${footprint.name}`}
              onClick={() =>
                summary && void transport.openApplication(summary.scanId, footprint.nodeId)
              }
            />
            <InspectorAction
              icon={<Trash2 />}
              label={`Uninstall ${footprint.name}…`}
              onClick={() => {
                window.location.hash = "/storage/applications";
              }}
            />
          </CardContent>
        </Card>
      </aside>

      <div className="min-w-0 space-y-4">
        <AppHeader footprint={footprint} icon={icon} />

        <Card className="shadow-none">
          <CardContent className="p-5">
            <div className="border-b pb-3">
              <h2 className="text-sm font-semibold">What's taking up space</h2>
              <p className="mt-1 text-xs text-muted-foreground">
                Files and data attributed to {footprint.name} by bundle identifier.
              </p>
            </div>
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
      </div>
    </div>
  );
}

function InspectorAction({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-2.5 rounded-md px-2 py-2 text-left text-xs transition-colors hover:bg-accent"
    >
      <span className="text-muted-foreground [&_svg]:size-4">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
    </button>
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
