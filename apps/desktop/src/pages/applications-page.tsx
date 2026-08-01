import { ArrowUpDown, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { ApplicationInventory, ApplicationItem } from "@nirmoka/transport";

import { EmptyState, MetricCard, PageHeader, SectionTitle } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useApp } from "@/lib/app-context";
import { formatBytes, formatCount } from "@/lib/format";

export function ApplicationsPage() {
  const { transport, scan } = useApp();
  const summary = scan.status === "done" ? scan.summary : null;
  const [inventory, setInventory] = useState<ApplicationInventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [largestFirst, setLargestFirst] = useState(true);

  useEffect(() => {
    if (!summary) {
      setInventory(null);
      return;
    }
    let live = true;
    setError(null);
    transport.applicationInventory(summary.scanId).then(
      (value) => live && setInventory(value),
      (reason: unknown) => live && setError(String(reason)),
    );
    return () => {
      live = false;
    };
  }, [summary, transport]);

  const rows = useMemo(() => {
    const found =
      inventory?.rows.filter((app) => app.name.toLowerCase().includes(search.toLowerCase())) ?? [];
    return largestFirst ? found : [...found].reverse();
  }, [inventory, largestFirst, search]);
  const totalBytes = inventory?.rows.reduce((sum, app) => sum + app.totalBytes, 0) ?? 0;

  return (
    <div className="space-y-6">
      <PageHeader title="Applications" subtitle="Application bundles found in the current scan" />
      {!summary ? (
        <EmptyState
          title="No application inventory"
          text="Scan /Applications, ~/Applications, or a parent directory to inventory real .app bundles."
        />
      ) : error ? (
        <p className="text-sm text-destructive">{error}</p>
      ) : (
        <>
          <div className="grid grid-cols-4 gap-3 max-[1100px]:grid-cols-2">
            <MetricCard
              label="Applications Found"
              value={formatCount(inventory?.total ?? 0)}
              hint={`Within ${summary.rootPath}`}
            />
            <MetricCard
              label="Scanned Footprint"
              value={formatBytes(totalBytes)}
              hint="Bundle contents only"
            />
            <MetricCard label="Last Used" value="Unavailable" hint="Not present in ncdu export" />
            <MetricCard
              label="Related Data / Leftovers"
              value="Unavailable"
              hint="No evidence-backed mapping available"
            />
          </div>
          <Card className="shadow-none">
            <CardContent className="p-5">
              <SectionTitle
                title="Applications by Size"
                action={
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setLargestFirst((value) => !value)}
                  >
                    <ArrowUpDown />
                    {largestFirst ? "Largest first" : "Smallest first"}
                  </Button>
                }
              />
              <div className="relative mb-3">
                <Search className="absolute left-3 top-2.5 size-4 text-muted-foreground" />
                <Input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search applications"
                  className="pl-9"
                />
              </div>
              {rows.length === 0 ? (
                <EmptyState
                  title="No .app bundles in this scan"
                  text="Nirmoka does not infer applications outside the directory you scanned."
                />
              ) : (
                <div className="divide-y">
                  {rows.map((app) => (
                    <ApplicationRow key={`${app.id}-${app.path}`} app={app} />
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
          {(inventory?.total ?? 0) > (inventory?.rows.length ?? 0) && (
            <p className="text-xs text-muted-foreground">
              Showing the largest {inventory?.rows.length} of {inventory?.total} bundles.
            </p>
          )}
        </>
      )}
    </div>
  );
}

function ApplicationRow({ app }: { app: ApplicationItem }) {
  return (
    <div className="flex items-center gap-3 py-3 text-sm">
      <span className="grid size-9 place-items-center rounded-lg bg-primary text-sm font-semibold text-primary-foreground">
        {app.name.slice(0, 1).toUpperCase()}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block font-medium">{app.name}</span>
        <span className="block truncate font-mono text-xs text-muted-foreground">{app.path}</span>
      </span>
      {app.sizeIsPartial && <span className="text-xs text-warning-foreground">Partial</span>}
      <span className="tabular-nums">{formatBytes(app.totalBytes)}</span>
    </div>
  );
}
