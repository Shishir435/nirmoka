import { Check, Clipboard, RefreshCw, Terminal } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import type { BackendSetup } from "@/lib/engine/backend-gating";

export function BackendSetupCard({
  setup,
  onCheckAgain,
  compact = false,
}: {
  setup: BackendSetup;
  onCheckAgain: () => Promise<void>;
  compact?: boolean;
}) {
  const [checking, setChecking] = useState(false);
  const ready = setup.state === "ready";

  const checkAgain = async () => {
    setChecking(true);
    try {
      await onCheckAgain();
    } finally {
      setChecking(false);
    }
  };

  const copy = async () => {
    if (!setup.command) return;
    try {
      await navigator.clipboard.writeText(setup.command);
      toast.success("Install command copied");
    } catch {
      toast.error("Could not copy the command");
    }
  };

  return (
    <Card className="shadow-none">
      <CardContent className={compact ? "p-4" : "p-5"}>
        <div className="flex items-start gap-4">
          <div
            className={
              ready
                ? "grid size-10 shrink-0 place-items-center rounded-xl bg-success/10 text-success"
                : "grid size-10 shrink-0 place-items-center rounded-xl bg-warning/10 text-warning-foreground"
            }
          >
            {ready ? <Check className="size-5" /> : <Terminal className="size-5" />}
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium">{setup.title}</p>
            <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{setup.detail}</p>

            {setup.command && (
              <div className="mt-3 flex items-center gap-2 rounded-lg border bg-background px-3 py-2.5">
                <code className="min-w-0 flex-1 overflow-x-auto font-mono text-xs">
                  {setup.command}
                </code>
              </div>
            )}

            {!ready && setup.state !== "checking" && (
              <div className="mt-3 flex items-center gap-3">
                {setup.command && (
                  <Button size="sm" onClick={copy} disabled={checking}>
                    <Clipboard />
                    Copy command
                  </Button>
                )}
                <Button variant="outline" size="sm" onClick={checkAgain} disabled={checking}>
                  <RefreshCw className={checking ? "animate-spin" : undefined} />
                  {checking ? "Checking…" : "Check again"}
                </Button>
              </div>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
