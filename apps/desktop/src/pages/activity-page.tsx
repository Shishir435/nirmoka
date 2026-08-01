import { RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";
import type { DeleteOperation } from "@nirmoka/transport";

import { EmptyState, PageHeader, SafetyBanner, StatusBadge } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { useApp } from "@/lib/app-context";

const dates = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export function ActivityPage() {
  const { transport } = useApp();
  const [operations, setOperations] = useState<DeleteOperation[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const load = () => {
    setError(null);
    transport.operationLog().then(setOperations, (reason: unknown) => setError(String(reason)));
  };
  useEffect(load, [transport]);

  const undo = async (operation: DeleteOperation) => {
    try {
      await transport.undoDelete(operation.id);
      load();
    } catch (reason) {
      setError(String(reason));
    }
  };

  return (
    <div className="space-y-6">
      <PageHeader
        title="Activity"
        subtitle="Durable deletion and undo history"
        action={
          <Button variant="outline" onClick={load}>
            <RotateCcw />
            Refresh
          </Button>
        }
      />
      {error && <p className="text-sm text-destructive">{error}</p>}
      {(operations?.length ?? 0) === 0 ? (
        <EmptyState
          title={operations ? "No recorded operations" : "Loading activity"}
          text="Scan history is not fabricated. Only durable deletion receipts appear here."
        />
      ) : (
        <Card className="shadow-none">
          <CardContent className="p-0">
            <div className="grid grid-cols-[90px_1fr_100px_110px_170px_90px] gap-4 border-b bg-muted/30 px-4 py-3 text-[11px] font-medium text-muted-foreground">
              <span>Backend</span>
              <span>Target</span>
              <span>Disposition</span>
              <span>Recovery</span>
              <span>Date & Time</span>
              <span>Status</span>
            </div>
            {operations?.map((operation) => (
              <div
                key={operation.id}
                className="grid grid-cols-[90px_1fr_100px_110px_170px_90px] items-center gap-4 border-b px-4 py-3 text-xs"
              >
                <span className="font-medium">{operation.backend}</span>
                <span className="truncate font-mono text-muted-foreground">
                  {operation.targetPath}
                </span>
                <span>{operation.disposition}</span>
                <span>{operation.recoverable ? "Recoverable" : "Permanent"}</span>
                <span className="text-muted-foreground">{dates.format(operation.deletedAtMs)}</span>
                {operation.recoverable && !operation.undone ? (
                  <Button size="sm" variant="outline" onClick={() => void undo(operation)}>
                    Undo
                  </Button>
                ) : (
                  <StatusBadge tone={operation.undone ? "neutral" : "success"}>
                    {operation.undone ? "Undone" : "Complete"}
                  </StatusBadge>
                )}
              </div>
            ))}
          </CardContent>
        </Card>
      )}
      <SafetyBanner compact>
        <p className="text-xs text-muted-foreground">
          The journal is stored locally. Sizes and item counts remain unavailable because existing
          receipts do not record them.
        </p>
      </SafetyBanner>
    </div>
  );
}
