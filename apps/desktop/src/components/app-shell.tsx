import type { ReactNode } from "react";
import { Brush, CircleHelp, HardDrive, History, Settings, ShieldCheck } from "lucide-react";

import { NirmokaMark } from "@/components/mark";
import { ScanBar, ScanStatusStrip } from "@/components/scan-controls";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useApp } from "@/lib/app-context";
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
  const { features } = useApp();
  // How much room the frame's own buttons take inside the webview, which is
  // zero wherever the frame keeps its own title bar — and zero until the
  // platform answers, so nothing is reserved on a guess. Anything above zero
  // means those buttons are sitting in this window's top-left corner and need a
  // row of their own.
  const inset = features?.windowControlsInset ?? 0;

  return (
    <TooltipProvider delayDuration={300}>
      <div className="bg-background flex h-screen min-h-160 overflow-hidden">
        {/* Opaque. A macOS vibrancy material was tried here and backed out: it
            needs `transparent: true`, which on macOS needs the
            `macos-private-api` feature — a private Apple API for a cosmetic
            effect, on a window whose reference design is not translucent. */}
        <aside className="bg-sidebar flex w-51 shrink-0 flex-col border-r max-[960px]:w-17 max-[960px]:items-center">
          {/* The title bar: the frame's buttons, then the name.

              With an overlay title bar macOS draws close, minimise, and zoom
              over this corner of the webview, so anything sharing the row starts
              after them. That was previously read as disqualifying — a logo 72px
              in hangs right of every nav item below it — so the buttons got the
              row to themselves and the brand went beneath. Two rows of chrome,
              the upper one visibly empty.

              But 72px into the title bar row is not a misaligned nav item. It is
              where the frame would have drawn the window title, which is what
              this is. One row, the name in the place the platform puts it, and
              the ~48px the separate brand block cost goes back to the nav.

              Keeps `border-b` and `h-15` so one rule still crosses the whole
              window. Where the frame keeps its own title bar the inset is zero
              and the padding falls back to the nav's own gutter. Below 961px the
              sidebar is narrower than the buttons themselves, so the name drops
              and the row is clearance again. */}
          <div
            data-tauri-drag-region
            style={inset > 0 ? { paddingLeft: inset } : undefined}
            className={cn(
              "flex h-15 w-full shrink-0 items-center gap-2 border-b",
              inset > 0 ? "pr-4" : "px-6",
            )}
          >
            <NirmokaMark className="size-6 shrink-0 rounded-md max-[960px]:hidden" />
            {/* The window frame no longer writes the name, so this is the only
                place it appears. It was in both, which is what looked wrong. */}
            <span data-tauri-drag-region className="text-[13px] font-semibold max-[960px]:hidden">
              Nirmoka
            </span>
          </div>
          <nav className="w-full space-y-1 px-3 pt-3 max-[960px]:px-2" aria-label="Main navigation">
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
        <div className="bg-background flex min-w-0 flex-1 flex-col">
          {/* Draggable on the header itself, not its children: the handler only
              fires when the mousedown target carries the attribute, so the scan
              input and the two buttons keep their own clicks while the padding
              and the gap between them move the window. Without an overlay title
              bar there is no frame left to drag. */}
          <header
            data-tauri-drag-region
            className="flex h-15 shrink-0 items-center gap-3 border-b bg-card/40 px-8 max-[960px]:px-5"
          >
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
            // A tinted fill with the icon in the accent colour, which is how a
            // sidebar marks its selection on this platform. A solid purple pill
            // with inverted text is a web navigation idiom, and at 44px wide it
            // was the heaviest element in the window.
            "flex w-full items-center gap-3 rounded-md px-3 py-1.5 text-[13px] transition-colors max-[960px]:justify-center max-[960px]:px-0",
            active
              ? "bg-primary/12 text-foreground font-medium"
              : "text-muted-foreground hover:bg-foreground/6 hover:text-foreground",
          )}
        >
          <Icon className={cn("size-4.5 shrink-0", active && "text-primary")} />
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
    // Onboarding is the one screen that renders outside the shell, so it paints
    // its own floor. This was `bg-muted/35` — a 35% tint over whatever sits
    // behind the window, which is now the desktop, because the frame carries a
    // vibrancy material. The opaque token is the same soft grey without the
    // see-through setup wizard.
    <div className="bg-muted grid min-h-screen place-items-center p-6">
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
