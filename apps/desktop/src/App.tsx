import { lazy, Suspense, useEffect, useState } from "react";
import { History, RefreshCw } from "lucide-react";
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
  firstLocation,
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
/**
 * Everything below the dashboard. Each renders as its own screen with a back
 * control rather than as a nav destination — see ADR 0031.
 */
const pages: Record<Exclude<Route, "storage">, React.LazyExoticComponent<React.ComponentType>> = {
  clean: lazy(() => import("@/pages/clean-page").then((module) => ({ default: module.CleanPage }))),
  activity: lazy(() =>
    import("@/pages/activity-page").then((module) => ({ default: module.ActivityPage })),
  ),
  help: lazy(() => import("@/pages/help-page").then((module) => ({ default: module.HelpPage }))),
};

/**
 * That a person has been shown the four introduction screens. Beside the theme
 * in `localStorage` rather than in the settings file, because it is a fact about
 * this window rather than about how Nirmoka is configured.
 */
const ONBOARDED_KEY = "nirmoka-onboarded";

const hasOnboarded = () => window.localStorage.getItem(ONBOARDED_KEY) === "true";

export function App() {
  const { isShell, backends, selection, chooseBackend, refreshBackends } = useApp();
  const [location, setLocation] = useState<Location | "onboarding">(() => {
    const opening = firstLocation(window.location.hash, hasOnboarded());
    // Written into the hash as well, so the listener below and the window agree
    // about where it is. Without this the first `hashchange` would navigate away
    // from a wizard the user had not finished.
    if (opening === "onboarding" && !window.location.hash) {
      window.location.hash = "#/onboarding";
    }
    return opening;
  });
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [refreshingBackends, setRefreshingBackends] = useState(false);
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
  const view: StorageView | null = location === "onboarding" ? null : location.view;
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
          <Onboarding
            onComplete={() => {
              window.localStorage.setItem(ONBOARDED_KEY, "true");
              go(DEFAULT_LOCATION);
            }}
          />
        </Suspense>
        <Toaster richColors position="bottom-right" />
      </>
    );
  const Page = location.route === "storage" ? null : pages[location.route];
  // The dashboard is the root, so everything else offers the way back to it.
  // A destination rather than a history step: it cannot strand anyone in a loop.
  const back =
    location.route === "storage" && location.view === null
      ? undefined
      : { label: "Nirmoka", onBack: () => go(DEFAULT_LOCATION) };
  return (
    <AppShell onSettings={() => setSettingsOpen(true)} onHelp={() => navigate("help")} back={back}>
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
              <div className="flex items-center justify-between gap-3">
                <label htmlFor="backend-choice" className="text-sm font-medium">
                  Preferred backend
                </label>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={refreshingBackends}
                  onClick={async () => {
                    setRefreshingBackends(true);
                    try {
                      await refreshBackends();
                    } finally {
                      setRefreshingBackends(false);
                    }
                  }}
                >
                  <RefreshCw className={refreshingBackends ? "animate-spin" : undefined} />
                  {refreshingBackends ? "Checking…" : "Check again"}
                </Button>
              </div>
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
            <div>
              <p className="text-sm font-medium">History</p>
              <p className="mt-1 text-xs text-muted-foreground">
                Every removal this program has made, newest first, across the Trash, cleanup runs,
                and deletions.
              </p>
              <Button
                variant="outline"
                className="mt-2"
                onClick={() => {
                  setSettingsOpen(false);
                  navigate("activity");
                }}
              >
                <History /> Open Activity
              </Button>
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
