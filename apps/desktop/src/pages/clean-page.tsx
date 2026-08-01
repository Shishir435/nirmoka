import { ShieldAlert } from "lucide-react";

import { EmptyState, PageHeader, SafetyBanner, StatusBadge } from "@/components/shared";
import { Card, CardContent } from "@/components/ui/card";
import { useApp } from "@/lib/app-context";

export function CleanPage() {
  const { backends } = useApp();
  const mole = backends?.find((backend) => backend.id === "mole");
  const installed = mole?.usable ?? false;
  const previewCapable =
    installed && mole?.capabilities.cleanupCategories && mole.capabilities.dryRun;

  return (
    <div className="space-y-6">
      <PageHeader title="Clean" subtitle="Backend-owned cleanup only" />
      <Card className="shadow-none">
        <CardContent className="p-6">
          <div className="flex items-start gap-4">
            <div className="grid size-11 shrink-0 place-items-center rounded-xl bg-warning/10 text-warning-foreground">
              <ShieldAlert className="size-5" />
            </div>
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <h2 className="font-medium">Mole cleanup categories</h2>
                <StatusBadge tone={previewCapable ? "success" : "neutral"}>
                  {previewCapable ? "Detected" : "Unavailable"}
                </StatusBadge>
              </div>
              <p className="max-w-2xl text-sm leading-relaxed text-muted-foreground">
                {previewCapable
                  ? "Mole reports cleanup and dry-run capabilities, but Nirmoka does not yet have a typed, tested bridge for its human-readable preview output. Cleanup execution is disabled rather than showing guessed categories, paths, or reclaimable bytes."
                  : installed
                    ? "This Mole version does not expose the capabilities required for an exact preview."
                    : "Install a supported Mole release to make its curated cleanup capabilities available. ncdu scans disks; it does not provide cleanup categories."}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
      <EmptyState
        title="No safe cleanup preview available"
        text="Nirmoka will only show exact backend-produced paths, sizes, counts, warnings, and recovery behavior. The current adapter API cannot provide those facts yet."
      />
      <SafetyBanner>
        <p className="text-sm font-medium">Failing closed</p>
        <p className="text-xs text-muted-foreground">
          No frontend delete command is constructed, and no Mole protection or cleanup tables are
          copied into Nirmoka.
        </p>
      </SafetyBanner>
    </div>
  );
}
