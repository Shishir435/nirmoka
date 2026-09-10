import { Check, HardDrive, ShieldCheck } from "lucide-react";

import { OnboardingLayout } from "@/components/app-shell";
import { BackendSetupCard } from "@/components/backend-setup-card";
import { NirmokaMark } from "@/components/mark";
import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { scannerSetup } from "@/lib/engine/backend-gating";

/**
 * First launch asks for the one thing required to deliver value: a scanner.
 * Backend choice, optional cleanup tools, and permissions are implementation
 * details or contextual decisions, so they do not belong in onboarding.
 */
export function Onboarding({ onComplete }: { onComplete: () => void }) {
  const { backends, refreshBackends, selection } = useApp();
  const setup = scannerSetup(backends, selection);
  const scanner = selection?.scanner
    ? backends?.find((backend) => backend.id === selection.scanner)
    : null;

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
          {setup.state === "ready" ? (
            <Ready
              scannerName={scanner?.displayName ?? selection?.scanner ?? "Scanner"}
              onComplete={onComplete}
            />
          ) : (
            <BackendSetupCard setup={setup} onCheckAgain={refreshBackends} />
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
