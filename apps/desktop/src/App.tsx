import { lazy, Suspense, useEffect, useState } from "react";
import { Toaster } from "sonner";

import { AppShell } from "@/components/app-shell";
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
import {
  DEFAULT_LOCATION,
  hashForLocation,
  locationFromHash,
  type Location,
  type Route,
  type StorageView,
} from "@/lib/engine/route";

const Onboarding = lazy(() =>
  import("@/pages/onboarding").then((module) => ({ default: module.Onboarding })),
);

const StoragePage = lazy(() =>
  import("@/pages/storage-page").then((module) => ({ default: module.StoragePage })),
);
const pages: Record<Exclude<Route, "storage">, React.LazyExoticComponent<React.ComponentType>> = {
  clean: lazy(() => import("@/pages/clean-page").then((module) => ({ default: module.CleanPage }))),
  activity: lazy(() =>
    import("@/pages/activity-page").then((module) => ({ default: module.ActivityPage })),
  ),
  help: lazy(() => import("@/pages/help-page").then((module) => ({ default: module.HelpPage }))),
};

export function App() {
  const { isShell, backends, selection, chooseBackend } = useApp();
  const [location, setLocation] = useState<Location | "onboarding">(() =>
    locationFromHash(window.location.hash),
  );
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [theme, setTheme] = useState<"light" | "dark">(() =>
    window.localStorage.getItem("nirmoka-theme") === "dark" ? "dark" : "light",
  );
  useEffect(() => {
    const sync = () => setLocation(locationFromHash(window.location.hash));
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);
  useEffect(() => {
    document.documentElement.classList.remove("light", "dark");
    document.documentElement.classList.add(theme);
  }, [theme]);
  // The view the user was last looking at, kept while they are on another
  // destination so returning to Storage does not silently reset it.
  const view: StorageView = location === "onboarding" ? DEFAULT_LOCATION.view : location.view;
  const go = (next: Location) => {
    window.location.hash = hashForLocation(next);
    setLocation(next);
  };
  const navigate = (route: Route) => go({ route, view });
  const chooseTheme = (next: "light" | "dark") => {
    document.documentElement.classList.remove("light", "dark");
    document.documentElement.classList.add(next);
    window.localStorage.setItem("nirmoka-theme", next);
    setTheme(next);
  };
  if (location === "onboarding")
    return (
      <>
        <Suspense
          fallback={
            // Opaque for the same reason `OnboardingLayout` is: this renders
            // outside the shell, and the window itself is transparent.
            <div className="bg-muted grid min-h-screen place-items-center">
              <Skeleton className="h-140 w-full max-w-155 rounded-[20px]" />
            </div>
          }
        >
          <Onboarding onComplete={() => go(DEFAULT_LOCATION)} />
        </Suspense>
        <Toaster richColors position="bottom-right" />
      </>
    );
  const Page = location.route === "storage" ? null : pages[location.route];
  return (
    <AppShell route={location.route} onRoute={navigate} onSettings={() => setSettingsOpen(true)}>
      {!isShell && (
        <div className="mb-5 rounded-lg border border-warning/30 bg-warning/10 px-4 py-2 text-xs text-warning-foreground">
          Browser development mode: fixture transport is active. Packaged Tauri builds always use
          the real transport.
        </div>
      )}
      <Suspense fallback={<PageLoading />}>
        {Page ? (
          <Page />
        ) : (
          <StoragePage view={view} onView={(next) => go({ route: "storage", view: next })} />
        )}
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
              <p className="text-sm font-medium">What this build can remove</p>
              <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
                Anything you select can be moved to the Trash, after a confirmation naming the path
                Nirmoka resolved, and the Finder's Put Back is the undo. Mole cleanup runs Mole's
                own command. Permanent deletion of a path you pick stays unavailable, because no
                current backend can bind execution to the filesystem object that was validated.
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
