import { ArrowUpDown, Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { ApplicationInventory, InstalledApplicationInventory } from "@nirmoka/transport";

import {
  EmptyState,
  MetricCard,
  PageHeader,
  SafetyBanner,
  SectionTitle,
} from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useApp } from "@/lib/app-context";
import { uninstallOffer } from "@/lib/engine/backend-gating";
import { formatBytes, formatCount } from "@/lib/format";

export function ApplicationsPage() {
  const { transport, scan, backends } = useApp();
  const offer = uninstallOffer(backends);
  const summary = scan.status === "done" ? scan.summary : null;
  const [inventory, setInventory] = useState<ApplicationInventory | null>(null);
  const [installed, setInstalled] = useState<InstalledApplicationInventory | null>(null);
  const [installedLoading, setInstalledLoading] = useState(true);
  const [installedError, setInstalledError] = useState<string | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [largestFirst, setLargestFirst] = useState(true);

  useEffect(() => {
    let live = true;
    transport
      .installedApplicationInventory()
      .then(
        (value) => {
          if (live) setInstalled(value);
        },
        (reason: unknown) => {
          if (live) setInstalledError(String(reason));
        },
      )
      .finally(() => {
        if (live) setInstalledLoading(false);
      });
    return () => {
      live = false;
    };
  }, [transport]);

  useEffect(() => {
    if (!summary) {
      setInventory(null);
      return;
    }
    let live = true;
    setScanError(null);
    transport.applicationInventory(summary.scanId).then(
      (value) => live && setInventory(value),
      (reason: unknown) => live && setScanError(String(reason)),
    );
    return () => {
      live = false;
    };
  }, [summary, transport]);

  const allRows = useMemo(
    () =>
      installed
        ? installed.rows.map((app) => ({
            key: `${app.bundleId}-${app.path}`,
            name: app.name,
            path: app.path,
            // Mole's own rounded label. Nothing here converts it to bytes: a
            // number derived from "410.9MB" would look exact and add up wrong.
            size: app.reportedSize,
            sizeIsPartial: false,
            detail: `${app.bundleId} · ${app.source}`,
            // The exact string Mole's uninstall command accepts. A display name
            // is not a command: "Google Chrome" is listed, "google-chrome" is
            // what the backend takes.
            uninstallName: app.uninstallName,
          }))
        : (inventory?.rows.map((app) => ({
            key: `${app.id}-${app.path}`,
            name: app.name,
            path: app.path,
            size: formatBytes(app.totalBytes),
            sizeIsPartial: app.sizeIsPartial,
            detail: null,
            // A scanned bundle is a directory, not a backend identifier.
            uninstallName: null,
          })) ?? []),
    [installed, inventory],
  );
  // Only the scan-derived rows carry byte counts, so only they can be ordered
  // by size. Mole's rows arrive ordered by path and stay that way: reversing
  // rounded text would produce an order that looks like a sort and is not one.
  const orderable = !installed;
  const rows = useMemo(() => {
    const found = allRows.filter((app) => app.name.toLowerCase().includes(search.toLowerCase()));
    return orderable && !largestFirst ? [...found].reverse() : found;
  }, [allRows, largestFirst, orderable, search]);
  // Only the scan knows byte counts. Mole publishes a rounded string per
  // application, and adding those up would be arithmetic on labels.
  const scannedBytes = installed
    ? null
    : (inventory?.rows.reduce((sum, app) => sum + app.totalBytes, 0) ?? 0);
  const total = installed?.total ?? inventory?.total ?? 0;
  const sourceHint = installed ? `Reported by ${installed.backend}` : `Within ${summary?.rootPath}`;

  return (
    <div className="space-y-6">
      <PageHeader
        title="Applications"
        subtitle={
          installed
            ? "Applications Mole can address for uninstall"
            : "Application bundles found in the current scan"
        }
      />
      {installedLoading && !inventory ? (
        <EmptyState title="Reading applications" text="Checking Mole and the current scan." />
      ) : !installed && !summary ? (
        <EmptyState
          title="No application inventory"
          text={`${installedError ?? "Mole application inventory is unavailable"}. Install a supported Mole release, or scan /Applications.`}
        />
      ) : scanError && !installed ? (
        <p className="text-sm text-destructive">{scanError}</p>
      ) : (
        <>
          <div className="grid grid-cols-4 gap-3 max-[1100px]:grid-cols-2">
            <MetricCard label="Applications Found" value={formatCount(total)} hint={sourceHint} />
            <MetricCard
              label={installed ? "Application Footprint" : "Scanned Footprint"}
              value={scannedBytes === null ? "Per application" : formatBytes(scannedBytes)}
              hint={
                scannedBytes === null
                  ? "Mole reports a rounded size per app, not a total"
                  : "Bundle contents only"
              }
            />
            <MetricCard label="Last Used" value="Unavailable" hint="Not reported by backend" />
            <MetricCard
              label="Related Data / Leftovers"
              value="Unavailable"
              hint="No evidence-backed mapping available"
            />
          </div>
          <Card className="shadow-none">
            <CardContent className="p-5">
              <SectionTitle
                title={orderable ? "Applications by Size" : "Applications by Path"}
                action={
                  orderable ? (
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setLargestFirst((value) => !value)}
                    >
                      <ArrowUpDown />
                      {largestFirst ? "Largest first" : "Smallest first"}
                    </Button>
                  ) : undefined
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
                  title={installed ? "No matching applications" : "No .app bundles in this scan"}
                  text={
                    installed
                      ? "Clear the search to see every application Mole reported."
                      : "Nirmoka does not infer applications outside the directory you scanned."
                  }
                />
              ) : (
                <div className="divide-y">
                  {rows.map((app) => (
                    <ApplicationRow key={app.key} app={app} />
                  ))}
                </div>
              )}
            </CardContent>
          </Card>
          {total > allRows.length && (
            <p className="text-xs text-muted-foreground">
              Showing {allRows.length} of {total} applications.
            </p>
          )}

          {installed && offer === "terminal" && (
            <SafetyBanner>
              <p className="text-sm font-medium">Uninstall runs in Terminal, not here</p>
              <p className="text-xs text-muted-foreground">
                {`${installed.backend} lists these applications and the exact name its uninstall
                command accepts, but every named uninstall stops at its own confirmation prompt and
                the release exposes no non-interactive flag. Nirmoka will not answer another tool's
                safety prompt for you, so run `}
                <code className="font-mono">mo uninstall &lt;name&gt;</code>
                {` yourself and confirm it there. Files go to the Trash unless you pass
                --permanent.`}
              </p>
            </SafetyBanner>
          )}
        </>
      )}
    </div>
  );
}

interface ApplicationRowModel {
  key: string;
  name: string;
  path: string;
  size: string;
  sizeIsPartial: boolean;
  detail: string | null;
  uninstallName: string | null;
}

function ApplicationRow({ app }: { app: ApplicationRowModel }) {
  return (
    <div className="flex items-center gap-3 py-3 text-sm">
      <span className="grid size-9 place-items-center rounded-lg bg-primary text-sm font-semibold text-primary-foreground">
        {app.name.slice(0, 1).toUpperCase()}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block font-medium">{app.name}</span>
        <span className="block truncate font-mono text-xs text-muted-foreground">{app.path}</span>
        {app.detail && (
          <span className="block truncate text-xs text-muted-foreground">{app.detail}</span>
        )}
        {app.uninstallName && app.uninstallName !== app.name && (
          <span className="block truncate text-xs text-muted-foreground">
            uninstall name: <code className="font-mono">{app.uninstallName}</code>
          </span>
        )}
      </span>
      {app.sizeIsPartial && <span className="text-xs text-warning-foreground">Partial</span>}
      <span className="tabular-nums">{app.size}</span>
    </div>
  );
}
