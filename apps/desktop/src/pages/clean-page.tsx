import {
  ChevronLeft,
  ChevronRight,
  History,
  Play,
  RefreshCw,
  ShieldAlert,
  Square,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useReducer, useState } from "react";

import type { CleanupOperation, CleanupPreview } from "@nirmoka/transport";

import {
  EmptyState,
  MetricCard,
  PageHeader,
  SafetyBanner,
  SectionTitle,
  StatusBadge,
} from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useApp } from "@/lib/app-context";
import { cleanupAvailability } from "@/lib/engine/backend-gating";
import {
  canReview,
  INITIAL_CLEANUP,
  outcomeLabel,
  outcomeTone,
  reduceCleanup,
} from "@/lib/engine/cleanup-flow";

const PAGE_SIZE = 50;

export function CleanPage() {
  const { backends, transport } = useApp();
  const cleanup = cleanupAvailability(backends);
  const previewCapable = cleanup.available;
  // Every transition lives in `cleanup-flow`, where the endings that matter —
  // stopped, failed, partial — are covered by tests instead of by clicking.
  const [flow, dispatch] = useReducer(reduceCleanup, INITIAL_CLEANUP);
  const { preview, preparation, running, result } = flow;
  const [page, setPage] = useState(0);
  const [history, setHistory] = useState<CleanupOperation[]>([]);

  const rows = useMemo(
    () =>
      preview?.categories.flatMap((category) =>
        category.items.map((item) => ({ category: category.name, ...item })),
      ) ?? [],
    [preview],
  );
  const pageCount = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const pageRows = rows.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);

  const loadHistory = useCallback(() => {
    transport.cleanupLog().then(setHistory, () => setHistory([]));
  }, [transport]);

  useEffect(loadHistory, [loadHistory]);

  const [stoppingPreview, setStoppingPreview] = useState(false);

  const loadPreview = () => {
    dispatch({ type: "previewStarted" });
    setStoppingPreview(false);
    setPage(0);
    transport.cleanupPreview().then(
      (value) => dispatch({ type: "previewArrived", preview: value }),
      (reason: unknown) => dispatch({ type: "previewFailed", message: String(reason) }),
    );
  };

  const cancelPreview = () => {
    setStoppingPreview(true);
    void transport.cancelCleanupPreview();
  };

  const review = () => {
    transport.prepareCleanup().then(
      (value) => dispatch({ type: "reviewed", preparation: value }),
      (reason: unknown) => dispatch({ type: "reviewFailed", message: String(reason) }),
    );
  };

  const run = (confirmationToken: number) => {
    dispatch({ type: "runStarted" });
    transport
      .confirmCleanup(confirmationToken)
      .then(
        (operation) => dispatch({ type: "runFinished", operation }),
        (reason: unknown) => dispatch({ type: "runFailed", message: String(reason) }),
      )
      .finally(loadHistory);
  };

  const stopRun = () => {
    dispatch({ type: "stopRequested" });
    void transport.cancelCleanup();
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title="Clean"
        subtitle="Review Mole’s current cleanup candidates"
        action={
          <Button
            onClick={flow.previewing ? cancelPreview : loadPreview}
            disabled={!previewCapable || stoppingPreview || running}
          >
            {flow.previewing ? <Square /> : <RefreshCw />}
            {stoppingPreview
              ? "Stopping preview"
              : flow.previewing
                ? "Cancel preview"
                : preview
                  ? "Refresh preview"
                  : "Generate preview"}
          </Button>
        }
      />

      <Card className="shadow-none">
        <CardContent className="p-6">
          <div className="flex items-start gap-4">
            <div className="grid size-11 shrink-0 place-items-center rounded-xl bg-warning/10 text-warning-foreground">
              <ShieldAlert className="size-5" />
            </div>
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <h2 className="font-medium">Mole cleanup preview</h2>
                <StatusBadge tone={previewCapable ? "success" : "neutral"}>
                  {previewCapable ? "Available" : "Unavailable"}
                </StatusBadge>
              </div>
              <p className="max-w-2xl text-sm leading-relaxed text-muted-foreground">
                {previewCapable
                  ? "Mole will inspect this Mac and publish the paths it currently considers eligible. Generating a preview removes nothing."
                  : cleanup.reason}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {flow.runError && <EmptyState title="Cleanup did not run" text={flow.runError} />}

      {result && <CleanupResult operation={result} />}

      {flow.previewError ? (
        <EmptyState title="Preview failed" text={flow.previewError} />
      ) : running ? (
        <EmptyState
          title="Mole is cleaning this Mac"
          text={
            flow.stopping
              ? "Stopping Mole. Anything it already removed stays removed."
              : "Mole is re-discovering candidates and removing what it finds. This can take several minutes."
          }
        />
      ) : !preview ? (
        <EmptyState
          title={flow.previewing ? "Mole is inspecting this Mac" : "No cleanup preview yet"}
          text={
            flow.previewing
              ? "This can take a few minutes. No files are being removed."
              : "Generate a fresh preview to see backend-produced categories, paths, sizes, and scope."
          }
        />
      ) : (
        <>
          <div className="grid grid-cols-4 gap-3 max-[1100px]:grid-cols-2">
            <MetricCard
              label="Potential Cleanup"
              value={preview.potentialCleanup ?? "None"}
              hint="Rounded value reported by Mole"
            />
            <MetricCard
              label="Items"
              value={preview.totalItems.toLocaleString()}
              hint="Mole-reported candidate count"
            />
            <MetricCard
              label="Categories"
              value={preview.categories.length.toLocaleString()}
              hint="Grouped by Mole"
            />
            <MetricCard
              label="System Scope"
              value={scopeLabel(preview.systemScope)}
              hint={
                preview.systemScope === "included"
                  ? "Admin access was available"
                  : "See preview warning"
              }
            />
          </div>

          {preview.warnings.map((warning) => (
            <SafetyBanner key={warning}>
              <p className="text-sm font-medium">Partial preview</p>
              <p className="text-xs text-muted-foreground">{warning}</p>
            </SafetyBanner>
          ))}

          <Card className="shadow-none">
            <CardContent className="p-5">
              <SectionTitle title="Backend-produced paths" />
              {pageRows.length === 0 ? (
                <EmptyState
                  title="Nothing significant to clean"
                  text="Mole’s current preview contains no eligible paths."
                />
              ) : (
                <div className="divide-y">
                  {pageRows.map((item, index) => {
                    const previous = pageRows[index - 1];
                    return (
                      <div key={`${item.category}-${item.path}`} className="py-3">
                        {previous?.category !== item.category && (
                          <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                            {item.category}
                          </p>
                        )}
                        <div className="flex items-center justify-between gap-4 text-sm">
                          <span className="min-w-0 truncate font-mono text-xs">{item.path}</span>
                          <span className="shrink-0 text-right tabular-nums">
                            {item.reportedSize ?? "Size unknown"}
                            {item.itemCount > 1 ? ` · ${item.itemCount} items` : ""}
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}

              {pageCount > 1 && (
                <div className="mt-4 flex items-center justify-between border-t pt-4">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setPage((value) => Math.max(0, value - 1))}
                    disabled={page === 0}
                  >
                    <ChevronLeft />
                    Previous
                  </Button>
                  <span className="text-xs text-muted-foreground">
                    Page {page + 1} of {pageCount} · {rows.length} paths
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setPage((value) => Math.min(pageCount - 1, value + 1))}
                    disabled={page + 1 >= pageCount}
                  >
                    Next
                    <ChevronRight />
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>

          <p className="text-xs text-muted-foreground">
            {`Preview generated by ${preview.backend} ${preview.backendVersion} at ${preview.generatedAt}. `}
            Mole will perform a fresh discovery before any future cleanup execution; a version
            change requires a new preview.
          </p>
        </>
      )}

      {running ? (
        <Button variant="outline" onClick={stopRun} disabled={flow.stopping}>
          <Square />
          {flow.stopping ? "Stopping Mole" : "Stop cleanup"}
        </Button>
      ) : (
        <Button onClick={review} disabled={!canReview(flow)}>
          <Play />
          Review and run cleanup
        </Button>
      )}

      <Dialog
        open={preparation !== null}
        onOpenChange={(open) => !open && dispatch({ type: "reviewDismissed" })}
      >
        <DialogContent>
          {preparation && (
            <>
              <DialogHeader>
                <DialogTitle>Run Mole cleanup?</DialogTitle>
                <DialogDescription>{preparation.warning}</DialogDescription>
              </DialogHeader>
              <dl className="space-y-2 text-sm">
                <ConfirmationFact
                  label="Backend"
                  value={`${preparation.backend} ${preparation.backendVersion}`}
                />
                <ConfirmationFact label="Reviewed at" value={preparation.previewGeneratedAt} />
                <ConfirmationFact
                  label="Reviewed items"
                  value={preparation.totalItems.toLocaleString()}
                />
                <ConfirmationFact
                  label="Reviewed size"
                  value={preparation.potentialCleanup ?? "Not reported"}
                />
                <ConfirmationFact
                  label="System scope"
                  value={scopeLabel(preparation.systemScope)}
                />
                <ConfirmationFact
                  label="Confirmation expires in"
                  value={`${preparation.expiresInSeconds}s`}
                />
              </dl>
              {preparation.warnings.map((warning) => (
                <p key={warning} className="mt-3 text-xs text-muted-foreground">
                  {warning}
                </p>
              ))}
              <div className="mt-6 flex justify-end gap-2">
                <Button variant="outline" onClick={() => dispatch({ type: "reviewDismissed" })}>
                  Keep reviewing
                </Button>
                <Button onClick={() => run(preparation.confirmationToken)}>
                  Run cleanup with Mole
                </Button>
              </div>
            </>
          )}
        </DialogContent>
      </Dialog>

      {history.length > 0 && (
        <Card className="shadow-none">
          <CardContent className="p-5">
            <SectionTitle
              title="Cleanup history"
              action={
                <Button variant="outline" size="sm" onClick={loadHistory}>
                  <History />
                  Reload
                </Button>
              }
            />
            <div className="divide-y">
              {history.map((operation) => (
                <div
                  key={operation.id}
                  className="flex items-center justify-between gap-4 py-3 text-sm"
                >
                  <div className="min-w-0">
                    <p className="truncate">
                      {new Date(operation.executedAtMs).toLocaleString()} ·{" "}
                      {`${operation.backend} ${operation.backendVersion}`}
                    </p>
                    <p className="mt-0.5 text-xs text-muted-foreground">
                      Reviewed {operation.reviewedItems.toLocaleString()} items ·{" "}
                      {scopeLabel(operation.systemScope)} scope
                      {operation.warnings.length > 0
                        ? ` · ${operation.warnings.length} backend warning${
                            operation.warnings.length === 1 ? "" : "s"
                          }`
                        : ""}
                    </p>
                  </div>
                  <StatusBadge tone={outcomeTone(operation.completion)}>
                    {outcomeLabel(operation.completion)}
                  </StatusBadge>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      <SafetyBanner>
        <p className="text-sm font-medium">Mole decides what is removed</p>
        <p className="text-xs text-muted-foreground">
          Running a cleanup calls Mole’s own command with no paths or categories from this window.
          Mole re-discovers candidates and applies its own protection rules; Nirmoka does not copy
          or recreate them, and cannot remove a path Mole did not choose.
        </p>
      </SafetyBanner>
    </div>
  );
}

function CleanupResult({ operation }: { operation: CleanupOperation }) {
  return (
    <Card className="shadow-none">
      <CardContent className="space-y-3 p-5">
        <div className="flex items-center gap-2">
          <h2 className="font-medium">Cleanup result</h2>
          <StatusBadge tone={outcomeTone(operation.completion)}>
            {outcomeLabel(operation.completion)}
          </StatusBadge>
        </div>
        <p className="text-sm leading-relaxed text-muted-foreground">
          {`${operation.backend} ${operation.backendVersion} `}
          {operation.completion === "cancelled"
            ? "was stopped part way through a run started at "
            : operation.completion === "failed"
              ? "failed part way through a run started at "
              : "ran at "}
          {new Date(operation.executedAtMs).toLocaleString()}
          {` with ${scopeLabel(operation.systemScope).toLowerCase()} scope. The review it was `}
          {`approved from listed ${operation.reviewedItems.toLocaleString()} items`}
          {operation.reviewedPotentialCleanup ? ` and ${operation.reviewedPotentialCleanup}` : ""}.
          Mole re-discovered candidates as it ran, so these are the numbers you approved, not a
          per-path receipt.
        </p>
        {operation.warnings.map((warning) => (
          <p key={warning} className="text-xs text-muted-foreground">
            {warning}
          </p>
        ))}
        {operation.logError && (
          <p className="text-xs text-warning-foreground">
            This run happened but could not be written to the operation journal:{" "}
            {operation.logError}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

function ConfirmationFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-right font-medium">{value}</dd>
    </div>
  );
}

function scopeLabel(scope: CleanupPreview["systemScope"]) {
  switch (scope) {
    case "included":
      return "Full";
    case "userOnly":
      return "User only";
    case "unknown":
      return "Unknown";
  }
}
