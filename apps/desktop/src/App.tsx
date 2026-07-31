import { useCallback, useEffect, useMemo, useState } from "react";

import { resolveTransport, type Backend, type BackendSelection } from "@nirmoka/transport";

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
  const [selection, setSelection] = useState<BackendSelection | null>(null);
  const [choosing, setChoosing] = useState(false);

  useEffect(() => {
    let active = true;

    Promise.all([transport.listBackends(), transport.backendSelection()])
      .then(([detected, resolved]) => {
        if (!active) return;
        setBackends(detected);
        setSelection(resolved);
      })
      .catch(() => {
        if (active) setBackends([]);
      });

    return () => {
      active = false;
    };
  }, [transport]);

  /**
   * The answer comes from Rust rather than being computed here.
   *
   * A preference is resolved against what each backend can actually do, and
   * duplicating that rule in the UI is how the two drift: the window would say
   * one backend and the scan would run on another. The round trip costs a
   * detection sweep, which is what the button's honesty is worth.
   */
  const choose = useCallback(
    (id: string | null) => {
      setChoosing(true);
      transport
        .chooseBackend(id)
        .then(setSelection)
        .finally(() => setChoosing(false));
    },
    [transport],
  );

  // Not `usable`, and no longer a capability check done here either: Rust has
  // already resolved which backend would run a scan, and `null` means nothing
  // installed can. Mole is usable on macOS, is its default, and cannot scan.
  const canScan = selection?.scanner != null;

  return (
    <main className="mx-auto flex min-h-screen max-w-3xl flex-col gap-8 px-6 py-12">
      <header className="space-y-2">
        <h1 className="text-2xl font-semibold tracking-tight">Nirmoka</h1>
        <p className="text-muted-foreground text-sm leading-relaxed">
          Disk analysis through the scanner you already have installed. The tree stays in Rust; this
          window asks for the rows it is about to paint.
        </p>
      </header>

      <ScanPanel transport={transport} enabled={canScan} />

      <section className="space-y-3">
        <h2 className="text-sm font-medium">Backends</h2>
        <BackendList backends={backends} selection={selection} onChoose={choose} busy={choosing} />
      </section>
    </main>
  );
}
