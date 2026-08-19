import type { ReactNode } from "react";
import { ArrowRight, CheckCircle2, Info, LockKeyhole, ShieldCheck } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

export function PageHeader({
  title,
  subtitle,
  action,
}: {
  title: string;
  subtitle: string;
  action?: ReactNode;
}) {
  return (
    <header className="flex min-h-12 items-start justify-between gap-6">
      <div>
        <h1 className="text-[22px] font-semibold tracking-[-0.02em]">{title}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
      </div>
      {action}
    </header>
  );
}

export function MetricCard({ label, value, hint }: { label: string; value: string; hint: string }) {
  return (
    <Card className="shadow-none">
      <CardContent className="p-4">
        <p className="text-xs text-muted-foreground">{label}</p>
        <p className="mt-2 text-xl font-semibold tracking-tight tabular-nums">{value}</p>
        <p className="mt-1 text-xs text-muted-foreground">{hint}</p>
      </CardContent>
    </Card>
  );
}

export function StatusBadge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "success" | "warning" | "neutral" | "purple";
}) {
  const styles = {
    success: "border-success/20 bg-success/10 text-success",
    warning: "border-warning/20 bg-warning/10 text-warning-foreground",
    neutral: "border-border bg-muted text-muted-foreground",
    purple: "border-primary/20 bg-primary/10 text-primary",
  };
  return (
    <Badge variant="outline" className={styles[tone]}>
      {children}
    </Badge>
  );
}

export function SafetyBanner({
  compact = false,
  children,
}: {
  compact?: boolean;
  children?: ReactNode;
}) {
  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-xl border border-primary/15 bg-accent/70 px-4",
        compact ? "py-2.5" : "py-4",
      )}
    >
      <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-background text-primary shadow-xs">
        <ShieldCheck className="size-5" />
      </div>
      <div className="min-w-0 flex-1">
        {children ?? (
          <>
            <p className="text-sm font-medium">Nothing is removed without your confirmation</p>
            <p className="text-xs text-muted-foreground">
              Removal moves things to the Trash, so the Finder can put them back. Permanent deletion
              of a path you pick is not offered.
            </p>
          </>
        )}
      </div>
      {!compact && (
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            window.location.hash = "/help";
          }}
        >
          Learn More
        </Button>
      )}
    </div>
  );
}

export function SectionTitle({ title, action }: { title: string; action?: ReactNode }) {
  return (
    <div className="mb-3 flex items-center justify-between">
      <h2 className="text-sm font-medium">{title}</h2>
      {action}
    </div>
  );
}

export function EmptyState({ title, text }: { title: string; text: string }) {
  return (
    <div className="grid min-h-56 place-content-center rounded-xl border border-dashed text-center">
      <Info className="mx-auto mb-3 size-6 text-muted-foreground" />
      <p className="text-sm font-medium">{title}</p>
      <p className="mt-1 text-xs text-muted-foreground">{text}</p>
    </div>
  );
}

export function OnboardingFeature({ title, text }: { title: string; text: string }) {
  return (
    <div className="flex gap-3">
      <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
      <div>
        <p className="text-sm font-medium">{title}</p>
        <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">{text}</p>
      </div>
    </div>
  );
}

export function RowAction() {
  return <ArrowRight className="size-3.5 text-primary" />;
}
export function PrivacyNote({ children }: { children: ReactNode }) {
  return (
    <p className="flex items-center justify-center gap-2 text-xs text-muted-foreground">
      <LockKeyhole className="size-3.5" />
      {children}
    </p>
  );
}
