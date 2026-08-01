import type { ReactNode } from "react";
import {
  AppWindow,
  Brush,
  CircleHelp,
  Code2,
  HardDrive,
  History,
  LayoutDashboard,
  Settings,
} from "lucide-react";

import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export type Route =
  "overview" | "clean" | "space" | "developer" | "applications" | "activity" | "help";

const primary = [
  ["overview", "Overview", LayoutDashboard],
  ["clean", "Clean", Brush],
  ["space", "Space Explorer", HardDrive],
  ["developer", "Developer", Code2],
  ["applications", "Applications", AppWindow],
  ["activity", "Activity", History],
] as const;

export function AppShell({
  route,
  onRoute,
  onSettings,
  children,
}: {
  route: Route;
  onRoute: (route: Route) => void;
  onSettings: () => void;
  children: ReactNode;
}) {
  return (
    <TooltipProvider delayDuration={300}>
      <div className="flex h-screen min-h-[640px] overflow-hidden bg-background">
        <aside className="flex w-[204px] shrink-0 flex-col border-r bg-sidebar px-3 py-5 max-[960px]:w-[68px] max-[960px]:items-center max-[960px]:px-2">
          <div className="mb-7 flex h-9 items-center gap-2 px-2">
            <div className="grid size-7 place-items-center rounded-lg bg-primary text-xs font-semibold text-primary-foreground">
              N
            </div>
            <span className="text-sm font-semibold max-[960px]:hidden">Nirmoka</span>
          </div>
          <nav className="w-full space-y-1" aria-label="Main navigation">
            {primary.map(([id, label, Icon]) => (
              <NavItem
                key={id}
                active={route === id}
                label={label}
                Icon={Icon}
                onClick={() => onRoute(id)}
              />
            ))}
          </nav>
          <div className="my-5 w-full border-t" />
          <p className="mb-2 w-full px-3 text-[11px] font-medium uppercase tracking-wider text-muted-foreground max-[960px]:sr-only">
            Support
          </p>
          <NavItem
            active={route === "help"}
            label="Help"
            Icon={CircleHelp}
            onClick={() => onRoute("help")}
          />
          <div className="mt-auto w-full space-y-1">
            <NavItem label="Settings" Icon={Settings} onClick={onSettings} />
            <div className="flex items-center gap-2 px-3 py-2 text-xs text-muted-foreground max-[960px]:justify-center max-[960px]:px-0">
              <span className="size-2 rounded-full bg-success" />
              <span className="max-[960px]:hidden">Read Only Mode</span>
            </div>
          </div>
        </aside>
        <main className="min-w-0 flex-1 overflow-y-auto">
          <div className="mx-auto min-h-full max-w-[1320px] px-8 py-7 max-[960px]:px-5">
            {children}
          </div>
        </main>
      </div>
    </TooltipProvider>
  );
}

function NavItem({
  active = false,
  label,
  Icon,
  onClick,
}: {
  active?: boolean;
  label: string;
  Icon: typeof LayoutDashboard;
  onClick: () => void;
}) {
  const button = (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-9 w-full items-center gap-3 rounded-lg px-3 text-left text-[13px] text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/20 max-[960px]:justify-center max-[960px]:px-0",
        active &&
          "bg-primary text-primary-foreground shadow-sm hover:bg-primary/90 hover:text-primary-foreground",
      )}
    >
      <Icon className="size-4" />
      <span className="max-[960px]:hidden">{label}</span>
    </button>
  );
  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent side="right" className="min-[961px]:hidden">
        {label}
      </TooltipContent>
    </Tooltip>
  );
}

export function OnboardingLayout({ step, children }: { step: number; children: ReactNode }) {
  return (
    <div className="grid min-h-screen place-items-center bg-muted/35 p-6">
      <section className="relative w-full max-w-[620px] rounded-[20px] border bg-card px-14 py-12 shadow-lg max-[700px]:px-7">
        {children}
        <div className="mt-10 flex justify-center gap-2" aria-label={`Step ${step} of 4`}>
          {[1, 2, 3, 4].map((n) => (
            <span
              key={n}
              className={cn("size-2 rounded-full bg-border", n === step && "bg-primary")}
            />
          ))}
        </div>
      </section>
    </div>
  );
}
