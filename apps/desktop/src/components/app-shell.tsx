import type { ReactNode } from "react";
import { ChevronLeft, CircleHelp, HardDrive, Settings } from "lucide-react";

import { ScanControl, ScanStatusStrip } from "@/components/scan-controls";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { useApp } from "@/lib/app-context";
import { cn } from "@/lib/utils";

/**
 * One window, one destination.
 *
 * The rail that carried Storage, Clean, and Activity is gone: the dashboard is
 * the window, and everything else is drilled into and backed out of. See
 * ADR 0031, which supersedes ADR 0026's navigation while keeping its reasoning
 * — a place you can only reach by picking it off a list was never a place.
 *
 * So the chrome is one row: the frame's own buttons, the name, and the three
 * controls that are not about the disk. `back` replaces the name when the
 * window is showing something below the dashboard, which is what every screen
 * in the approved design does.
 */
export function AppShell({
  onSettings,
  onHelp,
  back,
  children,
}: {
  onSettings: () => void;
  onHelp: () => void;
  /** Where the window is, and how to leave it. Absent on the dashboard. */
  back?: { label: string; onBack: () => void };
  children: ReactNode;
}) {
  const { features } = useApp();
  // How much room the frame's own buttons take inside the webview, which is
  // zero wherever the frame keeps its own title bar — and zero until the
  // platform answers, so nothing is reserved on a guess.
  const inset = features?.windowControlsInset ?? 0;

  return (
    <TooltipProvider delayDuration={300}>
      <div className="bg-background flex h-screen min-h-160 flex-col overflow-hidden">
        {/* Draggable on the header itself, not its children: the handler only
            fires when the mousedown target carries the attribute, so the
            controls keep their own clicks while the padding between them moves
            the window.

            Three columns rather than a flex row, so the name sits in the centre
            of the window rather than in the centre of whatever is left over.
            The outer columns are the same width, which is what keeps it there
            when one side has a back control and the other has three buttons.

            44px tall, not 56, and the frame's buttons are positioned to match
            rather than the other way round. `trafficLightPosition` in
            `tauri.conf.json` sets them level with the text beside them —
            measured against a screenshot, because the offset macOS applies is
            not a number this window can read back. Shrinking the
            header to chase wherever macOS put them was solving it with the one
            number this window does not own. That config is read when the window
            is created, so a running dev server has to be restarted before a
            change to it shows.

            Everything in here is `sm` because 44px is the budget. */}
        <header
          data-tauri-drag-region
          className="grid h-11 shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-3 border-b px-3"
          style={inset > 0 ? { paddingLeft: inset } : undefined}
        >
          <div className="flex min-w-0 items-center">
            {back ? (
              // `ml-2` rather than nothing: the inset ends exactly where the
              // frame's last button ends, so a control starting at it has its
              // hover fill running up against the zoom button. The gap is for
              // the fill, not the glyph.
              <Button variant="ghost" size="sm" className="ml-2" onClick={back.onBack}>
                <ChevronLeft /> {back.label}
              </Button>
            ) : null}
          </div>

          {/* The window frame no longer writes the name, so this is the only
              place it appears. The mark is not beside it: the application icon
              already sits in the Dock and in the switcher, and repeating it here
              is a logo in a title bar. Hidden entirely behind a back control,
              which names where the window is instead. */}
          <div data-tauri-drag-region className="flex items-center">
            {back ? null : (
              <span data-tauri-drag-region className="text-[13px] font-semibold">
                Nirmoka
              </span>
            )}
          </div>

          {/* Pulled out by the icon buttons' own padding, so the glyphs line up
              with the right edge of the content below rather than sitting a
              button's worth of padding inside it. */}
          {/* One button and two icons, not three boxes. Scan is the only thing
              here a person came to press; giving Settings and Help the same
              border made the title bar a row of equal-weight controls and drew
              the eye to the two that are housekeeping. Scan is outline rather
              than filled for the same reason — it was a purple primary, which
              made a once-per-session button the loudest object on screen. */}
          <div className="flex items-center justify-end gap-1.5">
            <ScanControl />
            <IconButton label="Settings" Icon={Settings} onClick={onSettings} />
            <IconButton label="Help" Icon={CircleHelp} onClick={onHelp} />
          </div>
        </header>

        <ScanStatusStrip />

        <main className="min-w-0 flex-1 overflow-y-auto">
          <div className="mx-auto min-h-full max-w-330 px-8 py-7 max-[960px]:px-5">{children}</div>
        </main>
      </div>
    </TooltipProvider>
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
          size="icon-sm"
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
