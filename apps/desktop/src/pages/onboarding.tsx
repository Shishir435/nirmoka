import {
  Check,
  CheckCircle2,
  Clipboard,
  LockKeyhole,
  ShieldCheck,
  Terminal,
  TriangleAlert,
} from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { OnboardingLayout } from "@/components/app-shell";
import { NirmokaMark } from "@/components/mark";
import { OnboardingFeature, PrivacyNote } from "@/components/shared";
import { Button } from "@/components/ui/button";
import { useApp } from "@/lib/app-context";
import { cn } from "@/lib/utils";

export function Onboarding({ onComplete }: { onComplete: () => void }) {
  const { backends, refreshBackends, selection } = useApp();
  const [step, setStep] = useState(1);
  const [checking, setChecking] = useState(false);
  const [access, setAccess] = useState<"standard" | "full">("standard");
  const [installMethod, setInstallMethod] = useState<"homebrew" | "manual">("homebrew");
  const installCommand =
    installMethod === "homebrew"
      ? "brew install ncdu mole"
      : "Install ncdu 2.x and Mole 1.48+ and add both binaries to PATH";
  const scanner = backends?.find((backend) => backend.capabilities.scan && backend.usable);
  const mole = backends?.find((backend) => backend.id === "mole");
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
          <h1 className="mt-7 text-2xl font-semibold">Check Disk Backends</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            ncdu scans on macOS. Mole provides separate cleanup capabilities.
          </p>
        </div>
        <div className="mt-8 rounded-xl border bg-muted/30 p-4">
          <div className="flex items-center gap-3">
            <TriangleAlert className="size-5 text-warning" />
            <div>
              <p className="text-sm font-medium">
                {scanner
                  ? `${scanner.displayName} scanner detected`
                  : "Supported scanner not found"}
              </p>
              <p className="text-xs text-muted-foreground">
                Mole:{" "}
                {mole?.usable
                  ? "detected"
                  : mole?.detection?.state === "unsupportedVersion"
                    ? "unsupported version"
                    : "not detected"}
              </p>
            </div>
          </div>
        </div>
        <div className="mt-5">
          <div className="flex gap-1 rounded-lg bg-muted p-1">
            <button
              onClick={() => setInstallMethod("homebrew")}
              className={cn(
                "h-8 flex-1 rounded-md text-xs font-medium",
                installMethod === "homebrew" ? "bg-background shadow-xs" : "text-muted-foreground",
              )}
            >
              Homebrew (Recommended)
            </button>
            <button
              onClick={() => setInstallMethod("manual")}
              className={cn(
                "h-8 flex-1 rounded-md text-xs font-medium",
                installMethod === "manual" ? "bg-background shadow-xs" : "text-muted-foreground",
              )}
            >
              Manual Install
            </button>
          </div>
          <p className="mt-4 text-xs font-medium">1. Open Terminal</p>
          <p className="mt-3 text-xs font-medium">2. Run Homebrew command</p>
          <div className="mt-2 flex items-center gap-2 rounded-lg border bg-background px-3 py-2.5 font-mono text-xs">
            <span className="flex-1">{installCommand}</span>
            <button
              aria-label="Copy command"
              onClick={() => {
                void navigator.clipboard?.writeText(installCommand);
                toast.success("Command copied");
              }}
            >
              <Clipboard className="size-4 text-muted-foreground" />
            </button>
          </div>
          <p className="mt-3 text-xs font-medium">3. Verify installation</p>
        </div>
        <div className="mt-8 flex justify-between">
          <Button variant="outline" onClick={() => setStep(1)}>
            Back
          </Button>
          <Button
            disabled={checking}
            onClick={async () => {
              setChecking(true);
              await refreshBackends();
              setChecking(false);
              setStep(3);
            }}
          >
            {checking ? "Looking for Mole…" : "Verify Installation"}
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
          <h1 className="mt-7 text-2xl font-semibold">Choose Access Level</h1>
          <p className="mt-2 text-sm text-muted-foreground">
            Nirmoka can scan more locations with your permission.
          </p>
        </div>
        <div className="mt-8 space-y-3">
          <AccessOption
            active={access === "standard"}
            title="Standard Access (Recommended)"
            text="Scan your home folder, Downloads, Applications, project folders and common caches."
            onClick={() => setAccess("standard")}
          />
          <AccessOption
            active={access === "full"}
            title="Full Disk Access (Optional)"
            text="Inspect additional protected app and Library locations for more complete results."
            onClick={() => setAccess("full")}
          />
        </div>
        {access === "full" && (
          <div className="mt-4 rounded-xl bg-muted p-4 text-xs text-muted-foreground">
            <p>Nirmoka remains usable without Full Disk Access.</p>
            <Button variant="outline" size="sm" className="mt-3" disabled>
              Open System Settings (Unavailable)
            </Button>
          </div>
        )}
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
          value={selection?.scanner ? `${selection.scanner} detected` : "No supported scanner"}
        />
        <ReadyRow label="Mole cleanup" value={mole?.usable ? "Detected" : "Unavailable"} />
        <ReadyRow
          label="Access Level"
          value={access === "standard" ? "Standard Access" : "Full Disk Access"}
        />
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
function AccessOption({
  active,
  title,
  text,
  onClick,
}: {
  active: boolean;
  title: string;
  text: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-4 rounded-xl border p-4 text-left focus-visible:ring-3 focus-visible:ring-ring/20",
        active && "border-primary bg-accent",
      )}
    >
      <span
        className={cn(
          "grid size-5 place-items-center rounded-full border",
          active && "border-primary bg-primary text-primary-foreground",
        )}
      >
        {active && <Check className="size-3" />}
      </span>
      <span className="flex-1">
        <span className="block text-sm font-medium">{title}</span>
        <span className="mt-1 block text-xs leading-relaxed text-muted-foreground">{text}</span>
      </span>
      <ShieldCheck className="size-4 text-muted-foreground" />
    </button>
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
