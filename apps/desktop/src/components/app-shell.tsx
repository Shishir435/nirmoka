import type { ReactNode } from "react";
import { Brush, CircleHelp, HardDrive, History, Settings, ShieldCheck } from "lucide-react";

import { NirmokaMark } from "@/components/mark";
import { ScanBar, ScanStatusStrip } from "@/components/scan-controls";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import type { Route } from "@/lib/engine/route";
import { cn } from "@/lib/utils";

/**
 * Three destinations, and the scan bar above all of them. Seven nav items
 * described the command surface rather than the work — see ADR 0026.
 */
const primary = [
  ["storage", "Storage", HardDrive],
  ["clean", "Clean", Brush],
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
      <div className="flex h-screen min-h-160 overflow-hidden bg-background">
        <aside className="flex w-51 shrink-0 flex-col border-r bg-sidebar max-[960px]:w-17 max-[960px]:items-center">
          {/* Same height as the header row, with the same rule under it, so one
              line crosses the whole window and the brand sits level with the
              scan bar rather than 8px below it. */}
          <div className="flex h-15 shrink-0 items-center gap-2 border-b px-5 max-[960px]:justify-center max-[960px]:px-2">
            <NirmokaMark className="size-7 shrink-0 rounded-lg" />
            <span className="text-sm font-semibold max-[960px]:hidden">Nirmoka</span>
          </div>
          <nav className="w-full space-y-1 px-3 pt-4 max-[960px]:px-2" aria-label="Main navigation">
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
          <div className="mt-auto w-full px-3 pb-5 max-[960px]:px-2">
            <Tooltip>
              <TooltipTrigger asChild>
                <div className="flex items-center gap-2 rounded-lg px-3 py-2 text-xs text-muted-foreground max-[960px]:justify-center max-[960px]:px-0">
                  <ShieldCheck className="size-4 shrink-0 text-success" />
                  <span className="max-[960px]:hidden">No permanent deletion</span>
                </div>
              </TooltipTrigger>
              <TooltipContent side="right" className="max-w-64">
                Removal goes to the Trash, so the Finder can put it back. Permanent deletion of a
                path you pick is not offered in this beta.
              </TooltipContent>
            </Tooltip>
          </div>
        </aside>
        <div className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-15 shrink-0 items-center gap-3 border-b bg-card/40 px-8 max-[960px]:px-5">
            <ScanBar />
            {/* Pulled out by the icon buttons' own padding, so the glyphs line
                up with the right edge of the content below rather than sitting
                a button's worth of padding inside it. */}
            <div className="-mr-2 flex shrink-0 items-center gap-0.5">
              <IconButton
                label="Help"
                Icon={CircleHelp}
                active={route === "help"}
                onClick={() => onRoute("help")}
              />
              <IconButton label="Settings" Icon={Settings} onClick={onSettings} />
            </div>
          </header>
          <ScanStatusStrip />
          <main className="min-w-0 flex-1 overflow-y-auto">
            <div className="mx-auto min-h-full max-w-330 px-8 py-7 max-[960px]:px-5">
              {children}
            </div>
          </main>
        </div>
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
  Icon: typeof HardDrive;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          aria-current={active ? "page" : undefined}
          className={cn(
            "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors max-[960px]:justify-center max-[960px]:px-0",
            active
              ? "bg-primary text-primary-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-foreground",
          )}
        >
          <Icon className="size-4.5 shrink-0" />
          <span className="max-[960px]:hidden">{label}</span>
        </button>
      </TooltipTrigger>
      <TooltipContent side="right" className="min-[961px]:hidden">
        {label}
      </TooltipContent>
    </Tooltip>
  );
}

export function OnboardingLayout({ step, children }: { step: number; children: ReactNode }) {
  return (
    <div className="grid min-h-screen place-items-center bg-muted/35 p-6">
      <section className="relative w-full max-w-155 rounded-[20px] border bg-card px-14 py-12 shadow-lg max-[700px]:px-7">
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

function IconButton({
  label,
  Icon,
  active = false,
  onClick,
}: {
  label: string;
  Icon: typeof HardDrive;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant={active ? "secondary" : "ghost"}
          size="icon"
          onClick={onClick}
          aria-label={label}
        >
          <Icon />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}
