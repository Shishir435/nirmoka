import { useEffect, useState } from "react";

import { createMockTransport, type Backend, type Transport } from "@nirmoka/transport";

/**
 * Step 0 placeholder.
 *
 * Deliberately wired through `Transport` rather than calling a backend
 * directly, so the boundary exists from the first screen. Today it is the mock;
 * from step 7 it becomes the Tauri implementation, and nothing in this file
 * changes.
 */

// From step 7: resolveTransport() picks the Tauri implementation at runtime.
const transport: Transport = createMockTransport();

export function App() {
  const [backends, setBackends] = useState<Backend[] | null>(null);

  useEffect(() => {
    let active = true;

    transport
      .listBackends()
      .then((result) => {
        if (active) setBackends(result);
      })
      .catch(() => {
        if (active) setBackends([]);
      });

    return () => {
      active = false;
    };
  }, []);

  return (
    <main className="mx-auto flex min-h-screen max-w-2xl flex-col justify-center gap-8 px-6 py-16">
      <header className="space-y-3">
        <p className="text-muted-foreground text-xs font-medium tracking-widest uppercase">
          Step 0 · Workspace skeleton
        </p>
        <h1 className="text-3xl font-semibold tracking-tight">Nirmoka</h1>
        <p className="text-muted-foreground text-sm leading-relaxed">
          A cross-platform desktop GUI for disk analysis and cleanup. Nothing is wired to a real
          backend yet — this screen reads through the transport boundary so the seam exists before
          there is anything behind it.
        </p>
      </header>

      <section className="space-y-3">
        <h2 className="text-sm font-medium">Backends (mock)</h2>

        {backends === null ? (
          <p className="text-muted-foreground text-sm">Detecting…</p>
        ) : backends.length === 0 ? (
          <p className="text-muted-foreground text-sm">No backends reported.</p>
        ) : (
          <ul className="divide-border divide-y overflow-hidden rounded-lg border">
            {backends.map((backend) => (
              <li key={backend.id} className="flex items-baseline justify-between px-4 py-3">
                <span className="text-sm font-medium">{backend.displayName}</span>
                <span className="text-muted-foreground font-mono text-xs">
                  {backend.detection?.state === "found"
                    ? backend.detection.version
                    : (backend.detection?.state ?? "unknown")}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <footer className="text-muted-foreground border-t pt-6 text-xs">
        Real detection lives in <code className="font-mono">nrmk backends</code> until the Tauri
        shell arrives in step 7.
      </footer>
    </main>
  );
}
