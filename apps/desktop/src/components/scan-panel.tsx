import { useCallback, useEffect, useState } from "react";

import type { Row, RowPage, ScanSummary, Transport, Unsubscribe } from "@nirmoka/transport";

import { Button } from "@/components/ui/button";
import { formatBytes, formatCount, plural } from "@/lib/format";

/** How many rows the first screen asks for. The tree view in step 8 replaces
 *  this with a virtualized window; until then a fixed page keeps invariant 5
 *  honest by construction. */
const PAGE = 20;

type ScanState =
  | { status: "idle" }
  | { status: "scanning"; root: string; scanned: number; currentPath: string }
  | { status: "done"; summary: ScanSummary }
  | { status: "stopped"; message: string }
  | { status: "failed"; message: string };

function Bar({ share }: { share: number }) {
  return (
    <div className="bg-muted h-1.5 w-24 overflow-hidden rounded-full">
      <div className="bg-brand h-full rounded-full" style={{ width: `${share * 100}%` }} />
    </div>
  );
}

/** Flags that mean "this number is not the whole story". Silence here would be
 *  a total that quietly omits what the backend could not read. */
function Flags({ row }: { row: Row }) {
  const notes = [
    row.readError && "unreadable",
    row.excluded && "excluded",
    row.hardlink && "hardlink",
  ].filter(Boolean);

  if (notes.length === 0) return null;

  return <span className="text-muted-foreground text-xs">{notes.join(" · ")}</span>;
}

function Rows({ page }: { page: RowPage }) {
  if (page.rows.length === 0) {
    return <p className="text-muted-foreground text-sm">Nothing under this directory.</p>;
  }

  return (
    <div className="space-y-2">
      <ul className="divide-border divide-y overflow-hidden rounded-lg border">
        {page.rows.map((row) => (
          <li key={row.id} className="flex items-center gap-4 px-4 py-2">
            <span className="flex-1 truncate font-mono text-sm">
              {row.name}
              {row.kind === "directory" ? "/" : ""}
            </span>
            <Flags row={row} />
            <Bar share={row.share} />
            <span className="w-20 text-right font-mono text-sm tabular-nums">
              {formatBytes(row.totalBytes)}
            </span>
          </li>
        ))}
      </ul>

      {page.total > page.rows.length && (
        <p className="text-muted-foreground text-xs">
          Showing {page.rows.length} of {plural(page.total, "entry", "entries")}. The rest stay in
          Rust until the tree view asks for them.
        </p>
      )}
    </div>
  );
}

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

export function ScanPanel({ transport, enabled }: { transport: Transport; enabled: boolean }) {
  const [path, setPath] = useState("");
  const [state, setState] = useState<ScanState>({ status: "idle" });
  const [page, setPage] = useState<RowPage | null>(null);
  /** Whether the event listeners are registered. No scan may start before they
   *  are: registration is a round trip into Rust, and a scan that finishes
   *  first would deliver its terminal event to nobody, leaving this stuck on
   *  "scanning" for a scan that already ended. */
  const [listening, setListening] = useState(false);

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
          setState({ status: "done", summary });
          // The window is requested only once the tree exists. Asking earlier
          // would be answered from a tree that is still being built.
          transport
            .rows(null, 0, PAGE)
            .then(setPage)
            .catch((error: unknown) => setState({ status: "failed", message: String(error) }));
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
    ]).then(() => {
      if (live) setListening(true);
    });

    return () => {
      live = false;
      setListening(false);
      off.forEach((unsubscribe) => unsubscribe());
    };
  }, [transport]);

  const start = useCallback(() => {
    setPage(null);
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
  const ready = enabled && listening;

  return (
    <section className="space-y-4">
      <div className="flex gap-2">
        <input
          value={path}
          onChange={(event) => setPath(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && path && !scanning && ready) start();
          }}
          placeholder="A directory to scan, e.g. /Users/you/Downloads"
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

      {state.status === "scanning" && (
        <div className="space-y-1">
          <p className="text-sm">
            Scanning {plural(state.scanned, "entry", "entries")}…{" "}
            <span className="text-muted-foreground">
              stopping kills the backend process, it does not just hide it
            </span>
          </p>
          <p className="text-muted-foreground truncate font-mono text-xs">{state.currentPath}</p>
        </div>
      )}

      {state.status === "stopped" && <p className="text-sm">{state.message}</p>}

      {state.status === "failed" && <p className="text-destructive text-sm">{state.message}</p>}

      {state.status === "done" && (
        <div className="space-y-4">
          <Summary summary={state.summary} />
          {page ? <Rows page={page} /> : <p className="text-muted-foreground text-sm">Loading…</p>}
        </div>
      )}
    </section>
  );
}
