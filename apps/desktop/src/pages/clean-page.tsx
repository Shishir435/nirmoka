import { ChevronLeft, ChevronRight, RefreshCw, ShieldAlert, Square } from "lucide-react";
import { useMemo, useState } from "react";

import type { CleanupPreview } from "@nirmoka/transport";

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
import { useApp } from "@/lib/app-context";

const PAGE_SIZE = 50;

export function CleanPage() {
  const { backends, transport } = useApp();
  const mole = backends?.find((backend) => backend.id === "mole");
  const installed = mole?.usable ?? false;
  const previewCapable =
    installed && mole?.capabilities.cleanupCategories && mole.capabilities.dryRun;
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [page, setPage] = useState(0);

  const rows = useMemo(
    () =>
      preview?.categories.flatMap((category) =>
        category.items.map((item) => ({ category: category.name, ...item })),
      ) ?? [],
    [preview],
  );
  const pageCount = Math.max(1, Math.ceil(rows.length / PAGE_SIZE));
  const pageRows = rows.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE);

  const loadPreview = () => {
    setLoading(true);
    setStopping(false);
    setError(null);
    setPage(0);
    transport
      .cleanupPreview()
      .then(setPreview, (reason: unknown) => setError(String(reason)))
      .finally(() => {
        setLoading(false);
        setStopping(false);
      });
  };

  const cancelPreview = () => {
    setStopping(true);
    void transport.cancelCleanupPreview();
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title="Clean"
        subtitle="Review Mole’s current cleanup candidates"
        action={
          <Button
            onClick={loading ? cancelPreview : loadPreview}
            disabled={!previewCapable || stopping}
          >
            {loading ? <Square /> : <RefreshCw />}
            {stopping
              ? "Stopping preview"
              : loading
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
                  : installed
                    ? "This Mole version does not expose the capabilities required for a cleanup preview."
                    : "Install a supported Mole release to use its curated cleanup rules. ncdu scans disks; it does not decide what is safe to clean."}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {error ? (
        <EmptyState title="Preview failed" text={error} />
      ) : !preview ? (
        <EmptyState
          title={loading ? "Mole is inspecting this Mac" : "No cleanup preview yet"}
          text={
            loading
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
            Preview generated by {preview.backend} at {preview.generatedAt}. Mole will perform a
            fresh discovery before any future cleanup execution.
          </p>
        </>
      )}

      <SafetyBanner>
        <p className="text-sm font-medium">Review only</p>
        <p className="text-xs text-muted-foreground">
          Cleanup execution remains disabled. Nirmoka displays Mole’s published paths and does not
          copy or recreate Mole’s cleanup or protection tables.
        </p>
      </SafetyBanner>
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
