import { Check, Clipboard, HardDrive, RefreshCw, ShieldCheck, Terminal } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { OnboardingLayout } from "@/components/app-shell";
import { NirmokaMark } from "@/components/mark";
import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { cn } from "@/lib/utils";

const INSTALL_COMMAND = "brew install ncdu";

/**
 * First launch asks for the one thing required to deliver value: a scanner.
 * Backend choice, optional cleanup tools, and permissions are implementation
 * details or contextual decisions, so they do not belong in onboarding.
 */
export function Onboarding({ onComplete }: { onComplete: () => void }) {
  const { backends, refreshBackends } = useApp();
  const [checking, setChecking] = useState(false);
  const scanner = backends?.find((backend) => backend.capabilities.scan && backend.usable);
  const detecting = backends === null;

  const checkAgain = async () => {
    setChecking(true);
    await refreshBackends();
    setChecking(false);
  };

  return (
    <OnboardingLayout step={1} steps={1}>
      <div className="mx-auto grid max-w-205 grid-cols-[minmax(0,1fr)_minmax(310px,.82fr)] items-center gap-12 max-[760px]:grid-cols-1 max-[760px]:gap-8">
        <section>
          <NirmokaMark className="size-14 rounded-[14px] shadow-sm" />
          <h1 className="mt-6 text-[30px] font-semibold tracking-tight">
            Find what is filling your Mac
          </h1>
          <p className="mt-2 max-w-lg text-sm leading-relaxed text-muted-foreground">
            Nirmoka turns a disk scan into a clear storage map, so you can find large files and
            folders without learning terminal commands.
          </p>

          <div className="mt-7 space-y-4">
            <Benefit
              icon={<HardDrive />}
              title="See where your space went"
              text="Start with your home folder, applications, downloads, or any folder you choose."
            />
            <Benefit
              icon={<ShieldCheck />}
              title="Safe by default"
              text="Scanning only reads. Nothing is removed unless you choose and confirm it."
            />
          </div>
        </section>

        <section className="rounded-2xl border bg-card p-6 shadow-xs">
          {scanner ? (
            <Ready scannerName={scanner.displayName} onComplete={onComplete} />
          ) : (
            <Setup detecting={detecting} checking={checking} onCheck={() => void checkAgain()} />
          )}
        </section>
      </div>
    </OnboardingLayout>
  );
}

function Benefit({ icon, title, text }: { icon: React.ReactNode; title: string; text: string }) {
  return (
    <div className="flex gap-3.5">
      <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-muted text-muted-foreground [&_svg]:size-4.5">
        {icon}
      </span>
      <div>
        <p className="text-sm font-medium">{title}</p>
        <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">{text}</p>
      </div>
    </div>
  );
}

function Ready({ scannerName, onComplete }: { scannerName: string; onComplete: () => void }) {
  return (
    <div className="text-center">
      <span className="mx-auto grid size-12 place-items-center rounded-full bg-success/12 text-success">
        <Check className="size-6" />
      </span>
      <h2 className="mt-4 text-lg font-semibold">Ready to scan</h2>
      <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
        {scannerName} is installed. Your first scan will be read-only.
      </p>
      <Button className="mt-6 w-full" onClick={onComplete}>
        Open Nirmoka
      </Button>
    </div>
  );
}

function Setup({
  detecting,
  checking,
  onCheck,
}: {
  detecting: boolean;
  checking: boolean;
  onCheck: () => void;
}) {
  const busy = detecting || checking;

  return (
    <div>
      <div className="flex items-center gap-3">
        <span className="grid size-10 place-items-center rounded-xl bg-foreground text-background">
          <Terminal className="size-5" />
        </span>
        <div>
          <h2 className="text-sm font-semibold">One quick setup</h2>
          <p className="mt-0.5 text-xs text-muted-foreground">
            {busy ? "Looking for a disk scanner…" : "Install the scanner Nirmoka uses."}
          </p>
        </div>
      </div>

      <ol className="mt-6 space-y-4 text-xs">
        <li>
          <StepNumber number={1} /> Open Terminal
        </li>
        <li>
          <span className="flex items-center">
            <StepNumber number={2} /> Paste this command
          </span>
          <div className="mt-2 flex items-center gap-2 rounded-lg border bg-muted/40 px-3 py-3 font-mono">
            <span className="min-w-0 flex-1 wrap-break-word">{INSTALL_COMMAND}</span>
            <button
              className="rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
              aria-label="Copy install command"
              onClick={() => {
                void navigator.clipboard?.writeText(INSTALL_COMMAND);
                toast.success("Command copied");
              }}
            >
              <Clipboard className="size-4" />
            </button>
          </div>
        </li>
        <li>
          <StepNumber number={3} /> Return here when it finishes
        </li>
      </ol>

      <Button variant="outline" className="mt-6 w-full" disabled={busy} onClick={onCheck}>
        <RefreshCw className={cn("size-4", busy && "animate-spin")} />
        {busy ? "Checking…" : "Check again"}
      </Button>
      <p className="mt-4 text-center text-[11px] leading-relaxed text-muted-foreground">
        Nirmoka does not bundle third-party command-line tools.
      </p>
    </div>
  );
}

function StepNumber({ number }: { number: number }) {
  return (
    <span className="mr-2 inline-grid size-5 place-items-center rounded-full bg-muted text-[10px] font-semibold text-muted-foreground">
      {number}
    </span>
  );
}
