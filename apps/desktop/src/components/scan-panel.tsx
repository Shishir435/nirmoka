import { useCallback, useEffect, useState } from "react";

import type { ScanSummary, Sort, Transport, Unsubscribe } from "@nirmoka/transport";

import { TreeView } from "@/components/tree-view";
import { Button } from "@/components/ui/button";
import { formatBytes, formatCount, plural } from "@/lib/format";

/** Registering the scan event listeners: a round trip into Rust, so it takes
 *  time and can fail. */
type Subscription =
  { status: "pending" } | { status: "ready" } | { status: "failed"; message: string };

type ScanState =
  | { status: "idle" }
  | { status: "scanning"; root: string; scanned: number; currentPath: string }
  | { status: "done"; summary: ScanSummary }
  | { status: "stopped"; message: string }
  | { status: "failed"; message: string };

/** Where in the finished tree the user is looking. `null` is the scan root. */
interface View {
  parentId: number | null;
  sort: Sort;
}

const START_AT: View = { parentId: null, sort: "largestFirst" };

function Summary({ summary }: { summary: ScanSummary }) {
  const warnings = [
    summary.readErrors > 0 && `${plural(summary.readErrors, "entry", "entries")} unreadable`,
    summary.excluded > 0 && `${formatCount(summary.excluded)} excluded`,
    summary.hardlinksDeduplicated > 0 &&
      `${plural(summary.hardlinksDeduplicated, "hardlink", "hardlinks")} counted once, saving ${formatBytes(summary.hardlinkBytesSaved)}`,
  ].filter(Boolean);

  return (
    <div className="space-y-1">
      <p className="text-sm">
        <span className="font-medium">{formatBytes(summary.totalBytes)}</span> across{" "}
        {plural(summary.entries, "entry", "entries")} in {formatCount(summary.directories)}{" "}
        directories.
      </p>
      {warnings.length > 0 && (
        <p className="text-muted-foreground text-xs">{warnings.join(" · ")}</p>
      )}
    </div>
  );
}

/**
 * A count and a path, and deliberately no percentage.
 *
 * The backend does not know how many entries it is going to find, so a progress
 * bar here would be an animation with a number attached to it. Counting up and
 * naming the directory being walked is both truthful and more useful: it is how
 * a user sees the scan is stuck on a network mount.
 */
function Progress({ scanned, currentPath }: { scanned: number; currentPath: string }) {
  return (
    <div className="space-y-1">
      <p className="text-sm">
        Scanning {plural(scanned, "entry", "entries")}…{" "}
        <span className="text-muted-foreground">
          stopping kills the backend process, it does not just hide it
        </span>
      </p>
      <p className="text-muted-foreground truncate font-mono text-xs">{currentPath}</p>
    </div>
  );
}

export function ScanPanel({ transport, enabled }: { transport: Transport; enabled: boolean }) {
  const [path, setPath] = useState("");
  const [state, setState] = useState<ScanState>({ status: "idle" });
  const [view, setView] = useState<View>(START_AT);
  const [subscription, setSubscription] = useState<Subscription>({ status: "pending" });
  /** Bumped to re-run the effect. Registration is a round trip that can fail;
   *  without a way to ask again, one failure would disable scanning until the
   *  window is reopened. */
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let live = true;
    const off: Unsubscribe[] = [];

    const register = (pending: Promise<Unsubscribe>) =>
      pending.then((unsubscribe) => {
        // The effect may have been torn down while this was in flight —
        // StrictMode does exactly that on every mount in development.
        if (live) off.push(unsubscribe);
        else unsubscribe();
      });

    void Promise.all([
      register(
        transport.onScanProgress((progress) =>
          setState((current) =>
            current.status === "scanning"
              ? { ...current, scanned: progress.scanned, currentPath: progress.currentPath }
              : current,
          ),
        ),
      ),

      register(
        transport.onScanFinished((summary) => {
          // A finished scan is a new tree with new node ids, so the view starts
          // over at its root. Keeping the old parent id would point into a tree
          // that no longer exists.
          setView(START_AT);
          setState({ status: "done", summary });
        }),
      ),

      register(
        transport.onScanFailed((failure) =>
          setState(
            failure.cancelled
              ? { status: "stopped", message: "Scan stopped." }
              : { status: "failed", message: failure.message },
          ),
        ),
      ),
    ]).then(
      () => {
        if (live) setSubscription({ status: "ready" });
      },
      (error: unknown) => {
        // Nothing will report a scan's progress or its end, so scanning stays
        // disabled — but silently disabled is a window that looks broken. Say
        // what happened and offer the retry.
        if (live) setSubscription({ status: "failed", message: String(error) });
      },
    );

    return () => {
      live = false;
      setSubscription({ status: "pending" });
      off.forEach((unsubscribe) => unsubscribe());
    };
  }, [transport, attempt]);

  const start = useCallback(() => {
    setView(START_AT);
    setState({ status: "scanning", root: path, scanned: 0, currentPath: path });

    transport
      .startScan(path)
      .then((root) =>
        setState((current) =>
          current.status === "scanning" ? { ...current, root, currentPath: root } : current,
        ),
      )
      .catch((error: unknown) => setState({ status: "failed", message: String(error) }));
  }, [path, transport]);

  const stop = useCallback(() => {
    void transport.cancelScan();
  }, [transport]);

  const scanning = state.status === "scanning";
  // A scan needs both a backend that can run it and somewhere for its events to
  // land.
  const ready = enabled && subscription.status === "ready";

  return (
    <section className="space-y-4">
      <div className="flex gap-2">
        {/* The example is `~/Downloads` rather than a `/Users/you/…` literal:
            the tilde is expanded on the Rust side, reads the same on every
            platform, and does not suggest a layout only macOS has. */}
        <input
          value={path}
          onChange={(event) => setPath(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && path && !scanning && ready) start();
          }}
          placeholder="A directory to scan, e.g. ~/Downloads"
          spellCheck={false}
          disabled={scanning}
          className="border-input bg-background focus-visible:ring-ring/50 h-9 flex-1 rounded-md border px-3 font-mono text-sm outline-none focus-visible:ring-[3px] disabled:opacity-50"
        />

        {scanning ? (
          <Button variant="destructive" onClick={stop}>
            Stop
          </Button>
        ) : (
          <Button onClick={start} disabled={!ready || path.length === 0}>
            Scan
          </Button>
        )}
      </div>

      {!enabled && (
        <p className="text-muted-foreground text-sm">
          No usable backend. Install ncdu 2.x and reopen this window.
        </p>
      )}

      {enabled && subscription.status === "failed" && (
        <div className="flex items-baseline gap-3">
          <p className="text-destructive text-sm">
            Could not subscribe to scan events: {subscription.message}
          </p>
          <Button variant="outline" size="sm" onClick={() => setAttempt((n) => n + 1)}>
            Try again
          </Button>
        </div>
      )}

      {state.status === "scanning" && (
        <Progress scanned={state.scanned} currentPath={state.currentPath} />
      )}

      {state.status === "stopped" && <p className="text-sm">{state.message}</p>}

      {state.status === "failed" && <p className="text-destructive text-sm">{state.message}</p>}

      {state.status === "done" && (
        <div className="space-y-4">
          <Summary summary={state.summary} />
          <TreeView
            transport={transport}
            summary={state.summary}
            parentId={view.parentId}
            sort={view.sort}
            onNavigate={(parentId) => setView((current) => ({ ...current, parentId }))}
            onSort={(sort) => setView((current) => ({ ...current, sort }))}
          />
        </div>
      )}
    </section>
  );
}
