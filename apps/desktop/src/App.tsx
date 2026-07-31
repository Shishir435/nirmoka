import { useEffect, useMemo, useState } from "react";

import { resolveTransport, type Backend } from "@nirmoka/transport";

import { BackendList } from "@/components/backend-list";
import { ScanPanel } from "@/components/scan-panel";

/**
 * Every backend call goes through `Transport`. Nothing in this tree knows that
 * Tauri exists — `resolveTransport()` hands back the real implementation inside
 * the shell and the mock in a plain browser, so `pnpm dev` on its own still
 * renders something to work on.
 */
export function App() {
  const transport = useMemo(() => resolveTransport(), []);
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
  }, [transport]);

  const usable = backends?.some((backend) => backend.usable) ?? false;

  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col gap-8 px-6 py-12">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold tracking-tight">Nirmoka</h1>
        <p className="text-muted-foreground text-sm leading-relaxed">
          Disk analysis through the scanner you already have installed. The tree stays in Rust; this
          window asks for the rows it is about to paint.
        </p>
      </header>

      <ScanPanel transport={transport} enabled={usable} />

      <section className="space-y-3">
        <h2 className="text-sm font-medium">Backends</h2>
        <BackendList backends={backends} />
      </section>
    </main>
  );
}
