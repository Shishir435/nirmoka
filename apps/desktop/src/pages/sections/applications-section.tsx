import { ArrowUpDown, Search, Trash2 } from "lucide-react";
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import type { ApplicationInventory, InstalledApplicationInventory } from "@nirmoka/transport";

import { EmptyState, MetricCard, SafetyBanner, SectionTitle } from "@/components/shared";
import { TrashConfirmation } from "@/components/trash-confirmation";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { useApp } from "@/lib/app-context";
import { uninstallOffer } from "@/lib/engine/backend-gating";
import { INITIAL_TRASH, outcomeMessage, reduceTrash } from "@/lib/engine/trash-flow";
import { formatBytes, formatCount } from "@/lib/format";

/**
 * The scan tree filtered to `.app` bundles, beside Mole's own inventory when a
 * scan cannot supply one. A view of the scan rather than a tab — see ADR 0026.
 */
export function ApplicationsSection() {
  const { transport, scan, backends, features } = useApp();
  const offer = uninstallOffer(backends);
  const summary = scan.status === "done" ? scan.summary : null;
  const [inventory, setInventory] = useState<ApplicationInventory | null>(null);
  const [installed, setInstalled] = useState<InstalledApplicationInventory | null>(null);
  const [installedLoading, setInstalledLoading] = useState(true);
  const [installedError, setInstalledError] = useState<string | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [largestFirst, setLargestFirst] = useState(true);
  const [trash, dispatchTrash] = useReducer(reduceTrash, INITIAL_TRASH);
  const nextTrashRequest = useRef(0);

  useEffect(() => {
    dispatchTrash({ type: "rescanned" });
  }, [summary?.scanId]);

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
            // Mole reports a path; the scan reports a node. Only a node can be
            // trashed, because only a node is something Rust can resolve
            // itself — see the banner below.
            nodeId: null,
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
            nodeId: app.id,
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
  const trashLabel = features?.trashLabel ?? "Move to Trash";

  const askToTrash = (nodeId: number) => {
    if (!summary) return;
    // One number per attempt — see the same counter in `tree-view.tsx`.
    const requestId = ++nextTrashRequest.current;
    dispatchTrash({ type: "prepareStarted", requestId, nodeId });
    transport.prepareTrash(summary.scanId, nodeId).then(
      (preparation) => dispatchTrash({ type: "prepared", requestId, preparation }),
      (reason: unknown) =>
        dispatchTrash({ type: "prepareFailed", requestId, message: String(reason) }),
    );
  };

  const doTrash = (confirmationToken: number) => {
    // Its own number, from the same counter — see `tree-view.tsx`.
    const requestId = ++nextTrashRequest.current;
    dispatchTrash({ type: "runStarted", requestId });
    transport.confirmTrash(confirmationToken).then(
      (operation) => dispatchTrash({ type: "trashed", requestId, operation }),
      (reason: unknown) => dispatchTrash({ type: "runFailed", requestId, message: String(reason) }),
    );
  };

  return (
    <div className="space-y-6">
      <p className="text-sm text-muted-foreground">
        {installed
          ? "Applications Mole can address for uninstall. Scan /Applications for bundles this window can move itself."
          : "Application bundles found in the current scan."}
      </p>
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
                    <ApplicationRow
                      key={app.key}
                      app={app}
                      trashLabel={trashLabel}
                      trashed={app.nodeId !== null && trash.trashedIds.includes(app.nodeId)}
                      busy={trash.preparing || trash.running || trash.preparation !== null}
                      onTrash={askToTrash}
                    />
                  ))}
                </div>
              )}
              {trash.error && <p className="mt-3 text-xs text-destructive">{trash.error}</p>}
              {trash.last && (
                <p className="mt-3 text-xs text-muted-foreground">{outcomeMessage(trash.last)}</p>
              )}
            </CardContent>
          </Card>
          {total > allRows.length && (
            <p className="text-xs text-muted-foreground">
              Showing {allRows.length} of {total} applications.
            </p>
          )}

          {installed ? (
            <SafetyBanner compact>
              <p className="text-xs text-muted-foreground">
                {`This list comes from ${installed.backend}, which reports a path rather than a
                position in a scan — and Nirmoka only moves things it can resolve itself, from its
                own tree. Scan `}
                <code className="font-mono">/Applications</code>
                {` to get the same bundles with a ${trashLabel} button on each one.`}
              </p>
            </SafetyBanner>
          ) : (
            <SafetyBanner compact>
              <p className="text-xs text-muted-foreground">
                {`${trashLabel} moves the application bundle and nothing else. Preferences, caches,
                and support files stay where they are — finding an application's leftovers is
                Mole's job, and `}
                <code className="font-mono">mo uninstall</code>
                {` cannot be driven past its own confirmation prompt. This is a smaller operation
                than an uninstall, and it does not pretend otherwise.`}
              </p>
            </SafetyBanner>
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

          <TrashConfirmation
            preparation={trash.preparation}
            label={trashLabel}
            onCancel={() => dispatchTrash({ type: "dismissed" })}
            onConfirm={doTrash}
          />
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
  /** Set only for scan-derived rows, which are the ones Rust can resolve. */
  nodeId: number | null;
}

function ApplicationRow({
  app,
  trashLabel,
  trashed,
  busy,
  onTrash,
}: {
  app: ApplicationRowModel;
  trashLabel: string;
  trashed: boolean;
  busy: boolean;
  onTrash: (nodeId: number) => void;
}) {
  // A local, so the button's callback keeps the narrowing: a property read
  // inside a closure does not, and the alternative is a `!` on the id that
  // decides which bundle moves.
  const nodeId = app.nodeId;

  return (
    <div className="flex items-center gap-3 py-3 text-sm">
      <span className="grid size-9 place-items-center rounded-lg bg-primary text-sm font-semibold text-primary-foreground">
        {app.name.slice(0, 1).toUpperCase()}
      </span>
      <span className="min-w-0 flex-1">
        <span
          className={`block font-medium ${trashed ? "text-muted-foreground line-through" : ""}`}
        >
          {app.name}
        </span>
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
      {nodeId !== null &&
        (trashed ? (
          <span className="shrink-0 text-xs text-muted-foreground">in the Trash</span>
        ) : (
          <Button
            variant="outline"
            size="sm"
            className="shrink-0 text-destructive hover:text-destructive"
            disabled={busy}
            onClick={() => onTrash(nodeId)}
            aria-label={`${trashLabel}: ${app.name}`}
          >
            <Trash2 />
          </Button>
        ))}
    </div>
  );
}
