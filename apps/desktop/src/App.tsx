import { lazy, Suspense, useEffect, useState } from "react";
import { Toaster } from "sonner";

import { AppShell, type Route } from "@/components/app-shell";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Skeleton } from "@/components/ui/skeleton";
import { useApp } from "@/lib/app-context";

const Onboarding = lazy(() =>
  import("@/pages/onboarding").then((module) => ({ default: module.Onboarding })),
);

const pages: Record<Route, React.LazyExoticComponent<React.ComponentType>> = {
  overview: lazy(() =>
    import("@/pages/overview-page").then((module) => ({ default: module.OverviewPage })),
  ),
  clean: lazy(() => import("@/pages/clean-page").then((module) => ({ default: module.CleanPage }))),
  space: lazy(() => import("@/pages/space-page").then((module) => ({ default: module.SpacePage }))),
  developer: lazy(() =>
    import("@/pages/developer-page").then((module) => ({ default: module.DeveloperPage })),
  ),
  applications: lazy(() =>
    import("@/pages/applications-page").then((module) => ({ default: module.ApplicationsPage })),
  ),
  activity: lazy(() =>
    import("@/pages/activity-page").then((module) => ({ default: module.ActivityPage })),
  ),
  help: lazy(() => import("@/pages/help-page").then((module) => ({ default: module.HelpPage }))),
};

function fromHash(): Route | "onboarding" {
  const value = window.location.hash.replace("#/", "");
  return value.startsWith("onboarding")
    ? "onboarding"
    : value in pages
      ? (value as Route)
      : "overview";
}

export function App() {
  const { isShell, backends, selection, chooseBackend } = useApp();
  const [route, setRoute] = useState<Route | "onboarding">(fromHash);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [theme, setTheme] = useState<"light" | "dark">(() =>
    window.localStorage.getItem("nirmoka-theme") === "dark" ? "dark" : "light",
  );
  useEffect(() => {
    const sync = () => setRoute(fromHash());
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);
  useEffect(() => {
    document.documentElement.classList.remove("light", "dark");
    document.documentElement.classList.add(theme);
  }, [theme]);
  const navigate = (next: Route) => {
    window.location.hash = `/${next}`;
    setRoute(next);
  };
  const chooseTheme = (next: "light" | "dark") => {
    document.documentElement.classList.remove("light", "dark");
    document.documentElement.classList.add(next);
    window.localStorage.setItem("nirmoka-theme", next);
    setTheme(next);
  };
  if (route === "onboarding")
    return (
      <>
        <Suspense
          fallback={
            <div className="grid min-h-screen place-items-center">
              <Skeleton className="h-140 w-full max-w-155 rounded-[20px]" />
            </div>
          }
        >
          <Onboarding onComplete={() => navigate("overview")} />
        </Suspense>
        <Toaster richColors position="bottom-right" />
      </>
    );
  const Page = pages[route];
  return (
    <AppShell route={route} onRoute={navigate} onSettings={() => setSettingsOpen(true)}>
      {!isShell && (
        <div className="mb-5 rounded-lg border border-warning/30 bg-warning/10 px-4 py-2 text-xs text-warning-foreground">
          Browser development mode: fixture transport is active. Packaged Tauri builds always use
          the real transport.
        </div>
      )}
      <Suspense fallback={<PageLoading />}>
        <Page />
      </Suspense>
      <Toaster richColors position="bottom-right" />
      <Dialog open={settingsOpen} onOpenChange={setSettingsOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>
              Appearance and backend status for this Nirmoka installation.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-5">
            <div>
              <p className="text-sm font-medium">Appearance</p>
              <div className="mt-2 flex gap-2">
                <Button
                  variant={theme === "light" ? "default" : "outline"}
                  onClick={() => chooseTheme("light")}
                >
                  Light
                </Button>
                <Button
                  variant={theme === "dark" ? "default" : "outline"}
                  onClick={() => chooseTheme("dark")}
                >
                  Dark
                </Button>
              </div>
            </div>
            <div className="rounded-xl border bg-muted/40 p-4">
              <p className="text-sm font-medium">Read Only Mode</p>
              <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
                Active. Selected-path deletion is unavailable in this beta because no current
                backend can bind execution to the validated filesystem object.
              </p>
            </div>
            <div>
              <label htmlFor="backend-choice" className="text-sm font-medium">
                Preferred backend
              </label>
              <select
                id="backend-choice"
                value={selection?.chosen ?? ""}
                onChange={(event) => void chooseBackend(event.target.value || null)}
                className="mt-2 h-9 w-full rounded-md border bg-background px-3 text-sm"
              >
                <option value="">Platform default</option>
                {backends?.map((backend) => (
                  <option key={backend.id} value={backend.id} disabled={!backend.usable}>
                    {backend.displayName} — {backend.usable ? "detected" : "unavailable"}
                  </option>
                ))}
              </select>
              {selection?.scannerInsteadOf && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {selection.scanner} scans because {selection.scannerInsteadOf} cannot scan.
                </p>
              )}
            </div>
            <div className="flex justify-end">
              <Button onClick={() => setSettingsOpen(false)}>Done</Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </AppShell>
  );
}

function PageLoading() {
  return (
    <div className="space-y-6" aria-label="Loading page">
      <Skeleton className="h-12 w-64" />
      <div className="grid grid-cols-4 gap-3">
        <Skeleton className="h-24" />
        <Skeleton className="h-24" />
        <Skeleton className="h-24" />
        <Skeleton className="h-24" />
      </div>
      <Skeleton className="h-105" />
    </div>
  );
}
