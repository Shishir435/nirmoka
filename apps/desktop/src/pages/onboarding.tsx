import { Check, CheckCircle2, LockKeyhole, Terminal } from "lucide-react";
import { useState } from "react";

import { OnboardingLayout } from "@/components/app-shell";
import { BackendSetupCard } from "@/components/backend-setup-card";
import { NirmokaMark } from "@/components/mark";
import { OnboardingFeature, PrivacyNote } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { scannerSetup } from "@/lib/engine/backend-gating";
import { cn } from "@/lib/utils";

export function Onboarding({ onComplete }: { onComplete: () => void }) {
  const { backends, refreshBackends, selection } = useApp();
  const [step, setStep] = useState(1);
  const scanner = scannerSetup(backends, selection);
  if (step === 1)
    return (
      <OnboardingLayout step={1}>
        <div className="text-center">
          <NirmokaMark className="mx-auto size-16 rounded-[18px] shadow-sm" />
          <h1 className="mt-7 text-2xl font-semibold">Welcome to Nirmoka</h1>
          <p className="mx-auto mt-2 max-w-sm text-sm leading-relaxed text-muted-foreground">
            A safe and powerful way to understand and clean your Mac storage.
          </p>
        </div>
        <div className="mx-auto mt-9 max-w-sm space-y-5">
          <OnboardingFeature
            title="Read-only by default"
            text="Scan and analyze without modifying anything."
          />
          <OnboardingFeature
            title="You are in control"
            text="Review everything before taking any action."
          />
          <OnboardingFeature
            title="Safe and transparent"
            text="Recoverable deletion and clear explanations."
          />
        </div>
        <Button className="mx-auto mt-10 flex w-64" onClick={() => setStep(2)}>
          Get Started
        </Button>
      </OnboardingLayout>
    );
  if (step === 2)
    return (
      <OnboardingLayout step={2}>
        <div className="text-center">
          <HeroIcon dark>
            <Terminal />
          </HeroIcon>
          <h1 className="mt-7 text-2xl font-semibold">Scanner Check</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            Nirmoka needs one supported scanner before it can map your storage.
          </p>
        </div>
        <div className="mt-8">
          <BackendSetupCard setup={scanner} onCheckAgain={refreshBackends} />
        </div>
        <p className="mt-4 text-center text-xs text-muted-foreground">
          Mole is optional. Nirmoka will offer it later when you open complete cleanup or uninstall.
        </p>
        <div className="mt-8 flex justify-between">
          <Button variant="outline" onClick={() => setStep(1)}>
            Back
          </Button>
          <Button disabled={scanner.state !== "ready"} onClick={() => setStep(3)}>
            Continue
          </Button>
        </div>
      </OnboardingLayout>
    );
  if (step === 3)
    return (
      <OnboardingLayout step={3}>
        <div className="text-center">
          <HeroIcon>
            <LockKeyhole />
          </HeroIcon>
          <h1 className="mt-7 text-2xl font-semibold">Start with Standard Access</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            Start safely. You can grant broader access later if a scan reports protected paths.
          </p>
        </div>
        <div className="mt-8 rounded-xl border bg-muted/30 p-5">
          <p className="text-sm font-medium">Standard Access</p>
          <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
            Scan your home folder, Downloads, Applications, project folders and common caches.
            Protected macOS locations may be reported as unreadable instead of being silently
            omitted.
          </p>
        </div>
        <PrivacyNote>Nirmoka never changes permissions itself.</PrivacyNote>
        <div className="mt-8 flex justify-between">
          <Button variant="outline" onClick={() => setStep(2)}>
            Back
          </Button>
          <Button onClick={() => setStep(4)}>Continue</Button>
        </div>
      </OnboardingLayout>
    );
  return (
    <OnboardingLayout step={4}>
      <div className="text-center">
        <div className="mx-auto grid size-16 place-items-center rounded-full bg-success text-white shadow-sm">
          <Check className="size-8" />
        </div>
        <h1 className="mt-7 text-2xl font-semibold">You're All Set!</h1>
        <p className="mt-2 text-sm text-muted-foreground">
          Nirmoka is ready to help you take control of your Mac storage.
        </p>
      </div>
      <div className="mx-auto mt-9 max-w-sm space-y-5">
        <ReadyRow
          label="Scanner"
          value={selection?.scanner ? `${selection.scanner} detected` : "Ready"}
        />
        <ReadyRow label="Access Level" value="Standard Access" />
        <ReadyRow label="Ready to Scan" value="Read-only first scan" />
      </div>
      <p className="mt-8 text-center text-xs text-muted-foreground">
        First scan does not delete or modify files.
      </p>
      <Button className="mx-auto mt-6 flex w-64" onClick={onComplete}>
        Open Nirmoka
      </Button>
    </OnboardingLayout>
  );
}
function HeroIcon({ children, dark = false }: { children: React.ReactNode; dark?: boolean }) {
  return (
    <div
      className={cn(
        "mx-auto grid size-16 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-lg [&_svg]:size-8",
        dark && "bg-foreground",
      )}
    >
      {children}
    </div>
  );
}
function ReadyRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center gap-3">
      <CheckCircle2 className="size-5 text-success" />
      <div>
        <p className="text-sm font-medium">{label}</p>
        <p className="text-xs text-muted-foreground">{value}</p>
      </div>
    </div>
  );
}
