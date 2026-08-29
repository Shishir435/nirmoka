import { ArrowUpDown, Search, Trash2 } from "lucide-react";
import { useEffect, useMemo, useReducer, useRef, useState } from "react";
import type { ApplicationInventory, InstalledApplicationInventory } from "@nirmoka/transport";

import { BackendSetupCard } from "@/components/backend-setup-card";
import { EmptyState, MetricCard, SafetyBanner, SectionTitle } from "@/components/shared";
import { TrashConfirmation } from "@/components/trash-confirmation";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { UninstallReview } from "@/components/uninstall-review";
import { useApp } from "@/lib/app-context";
import { moleSetup, uninstallOffer } from "@/lib/engine/backend-gating";
import { INITIAL_TRASH, outcomeMessage, reduceTrash } from "@/lib/engine/trash-flow";
import {
  canUninstall,
  INITIAL_UNINSTALL,
  reduceUninstall,
  uninstallOutcomeMessage,
} from "@/lib/engine/uninstall-flow";
import { formatBytes, formatCount } from "@/lib/format";

/**
 * The scan tree filtered to `.app` bundles, beside Mole's own inventory when a
 * scan cannot supply one. A view of the scan rather than a tab — see ADR 0026.
 */
/**
 * Icons for a list of bundles, by path.
 *
 * Mole's inventory has no scan behind it, so there is no node id to ask with —
 * see `applicationIconAt`. Decoration throughout: a bundle whose icon cannot be
 * read keeps the lettered tile, and nothing here can fail the list.
 */
function useApplicationIcons(paths: string[]) {
  const { transport } = useApp();
  const [icons, setIcons] = useState<Record<string, string>>({});
  const key = paths.join("\u0000");

  useEffect(() => {
    let live = true;
    for (const path of key.split("\u0000").filter(Boolean)) {
      transport
        .applicationIconAt(path)
        .then((icon) => {
          if (live && icon) setIcons((current) => ({ ...current, [path]: icon }));
        })
        .catch(() => {
          // Decoration.
        });
    }
    return () => {
      live = false;
    };
  }, [key, transport]);

  return icons;
}

export function ApplicationsSection() {
  const { transport, scan, backends, features, refreshBackends } = useApp();
  const offer = uninstallOffer(backends);
  const mole = moleSetup(backends);
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
  const [uninstall, dispatchUninstall] = useReducer(reduceUninstall, INITIAL_UNINSTALL);
  const nextUninstallRequest = useRef(0);

  useEffect(() => {
    dispatchTrash({ type: "rescanned" });
  }, [summary?.scanId]);

  useEffect(() => {
    if (mole.state !== "ready") {
      setInstalled(null);
      setInstalledError(null);
      setInstalledLoading(false);
      return;
    }

    let live = true;
    setInstalledLoading(true);
    setInstalledError(null);
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
  }, [mole.state, transport]);

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
  // Bounded: the list is searchable, and without a cap a long one would read a
  // file per row on every keystroke. What is on screen is what gets an icon.
  const iconPaths = useMemo(() => rows.slice(0, 40).map((app) => app.path), [rows]);
  const icons = useApplicationIcons(iconPaths);
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

  /**
   * Ask the backend what removing this application would do.
   *
   * Removes nothing. The identifier is the backend's own `uninstallName`, and
   * Rust checks it against the live inventory before it becomes an argument.
   */
  const askToUninstall = (name: string) => {
    // Its own counter, for the same reason the Trash flow has one: a review takes
    // seconds and the user can click another row while it runs.
    const requestId = ++nextUninstallRequest.current;
    dispatchUninstall({ type: "reviewStarted", requestId, name });
    transport.uninstallPreview([name]).then(
      (preview) => dispatchUninstall({ type: "reviewed", requestId, preview }),
      (reason: unknown) =>
        dispatchUninstall({ type: "reviewFailed", requestId, message: String(reason) }),
    );
  };

  /** The plan was read. Bind it to a token, which is all that can start the run. */
  const approveUninstall = () => {
    const requestId = nextUninstallRequest.current;
    transport.prepareUninstall().then(
      (preparation) => dispatchUninstall({ type: "prepared", requestId, preparation }),
      (reason: unknown) =>
        dispatchUninstall({ type: "reviewFailed", requestId, message: String(reason) }),
    );
  };

  const doUninstall = (confirmationToken: number) => {
    const requestId = ++nextUninstallRequest.current;
    dispatchUninstall({ type: "runStarted", requestId });
    transport.confirmUninstall(confirmationToken).then(
      (operation) => dispatchUninstall({ type: "removed", requestId, operation }),
      (reason: unknown) =>
        dispatchUninstall({ type: "runFailed", requestId, message: String(reason) }),
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
        <div className="space-y-4">
          <EmptyState
            title="No application inventory"
            text={`${installedError ?? "Scan /Applications to list application bundles"}. Complete uninstall is an optional Mole capability.`}
          />
          {mole.state !== "ready" && (
            <BackendSetupCard setup={mole} onCheckAgain={refreshBackends} compact />
          )}
        </div>
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
                      icon={icons[app.path]}
                      trashLabel={trashLabel}
                      trashed={app.nodeId !== null && trash.trashedIds.includes(app.nodeId)}
                      busy={trash.preparing || trash.running || trash.preparation !== null}
                      onTrash={askToTrash}
                      // Offered only where the backend can actually do it, and
                      // only for rows carrying its own identifier: a scanned
                      // bundle is a directory, not something `mo uninstall` takes.
                      removable={offer === "app" && canUninstall(uninstall, app.uninstallName)}
                      uninstalled={
                        app.uninstallName !== null &&
                        uninstall.removedNames.some(
                          (name) => name === app.uninstallName || name === app.name,
                        )
                      }
                      reviewing={uninstall.reviewing && uninstall.activeName === app.uninstallName}
                      onUninstall={askToUninstall}
                    />
                  ))}
                </div>
              )}
              {trash.error && <p className="mt-3 text-xs text-destructive">{trash.error}</p>}
              {trash.last && (
                <p className="mt-3 text-xs text-muted-foreground">{outcomeMessage(trash.last)}</p>
              )}
              {uninstall.error && (
                <p className="mt-3 text-xs text-destructive">{uninstall.error}</p>
              )}
              {uninstall.last && (
                <p className="mt-3 text-xs text-muted-foreground">
                  {uninstallOutcomeMessage(uninstall.last)}
                </p>
              )}
            </CardContent>
          </Card>
          {total > allRows.length && (
            <p className="text-xs text-muted-foreground">
              Showing {allRows.length} of {total} applications.
            </p>
          )}

          {!installed && mole.state !== "ready" && (
            <BackendSetupCard setup={mole} onCheckAgain={refreshBackends} compact />
          )}

          {installed ? (
            <SafetyBanner compact>
              <p className="text-xs text-muted-foreground">
                {offer === "app"
                  ? `Uninstall asks ${installed.backend} what it would remove, shows you that exact
                     plan, and runs it only after you approve. It decides what to remove and applies
                     its own protections; files go to the Trash. Scan `
                  : `This list comes from ${installed.backend}, which reports a path rather than a
                     position in a scan — and Nirmoka only moves things it can resolve itself, from
                     its own tree. Scan `}
                <code className="font-mono">/Applications</code>
                {` to get the same bundles with a ${trashLabel} button on each one.`}
              </p>
            </SafetyBanner>
          ) : (
            <SafetyBanner compact>
              <p className="text-xs text-muted-foreground">
                {`${trashLabel} moves the application bundle and nothing else. Preferences, caches,
                and support files stay where they are — finding an application's leftovers is
                Mole's job. Use Uninstall on the list Mole publishes for the complete removal; this
                is a smaller operation, and it does not pretend otherwise.`}
              </p>
            </SafetyBanner>
          )}

          {installed && offer === "terminal" && (
            <SafetyBanner>
              <p className="text-sm font-medium">Uninstall runs in Terminal, not here</p>
              <p className="text-xs text-muted-foreground">
                {`${installed.backend} lists these applications and the exact name its uninstall
                command accepts, but this release cannot produce the plan Nirmoka shows you before
                removing anything — and it will not run a removal it cannot describe first. Run `}
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

          <UninstallReview
            preview={uninstall.preview}
            preparation={uninstall.preparation}
            running={uninstall.running}
            onCancel={() => dispatchUninstall({ type: "dismissed" })}
            onApprove={approveUninstall}
            onConfirm={doUninstall}
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
  icon,
  trashLabel,
  trashed,
  busy,
  onTrash,
  removable,
  uninstalled,
  reviewing,
  onUninstall,
}: {
  app: ApplicationRowModel;
  /** A `data:` URL, or undefined where the bundle has no readable icon. */
  icon?: string;
  trashLabel: string;
  trashed: boolean;
  busy: boolean;
  onTrash: (nodeId: number) => void;
  removable: boolean;
  uninstalled: boolean;
  reviewing: boolean;
  onUninstall: (name: string) => void;
}) {
  // A local, so the button's callback keeps the narrowing: a property read
  // inside a closure does not, and the alternative is a `!` on the id that
  // decides which bundle moves.
  const nodeId = app.nodeId;
  // Same reasoning for the identifier that decides which application is removed.
  const uninstallName = app.uninstallName;

  return (
    <div className="flex items-center gap-3 py-3 text-sm">
      {/* The application's own icon where it has one. The lettered tile is the
          fallback, not the design: a list of coloured initials is what a window
          shows when it cannot read what it is listing. */}
      {icon ? (
        <img src={icon} alt="" className="size-9 shrink-0 rounded-lg" />
      ) : (
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-primary text-sm font-semibold text-primary-foreground">
          {app.name.slice(0, 1).toUpperCase()}
        </span>
      )}
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
      {uninstallName !== null &&
        (uninstalled ? (
          <span className="shrink-0 text-xs text-muted-foreground">uninstalled</span>
        ) : (
          <Button
            variant="outline"
            size="sm"
            className="shrink-0"
            disabled={!removable}
            onClick={() => onUninstall(uninstallName)}
          >
            {reviewing ? "Checking…" : "Uninstall"}
          </Button>
        ))}
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
