import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import {
  isTauri,
  resolveTransport,
  type Backend,
  type BackendSelection,
  type ScanProgress,
  type ScanSummary,
  type Transport,
  type Unsubscribe,
} from "@nirmoka/transport";

export type ScanState =
  | { status: "idle" }
  | { status: "scanning"; root: string; progress: ScanProgress }
  | { status: "done"; summary: ScanSummary }
  | { status: "cancelled" }
  | { status: "failed"; message: string };

interface AppContextValue {
  transport: Transport;
  isShell: boolean;
  backends: Backend[] | null;
  selection: BackendSelection | null;
  backendError: string | null;
  listenersReady: boolean;
  scan: ScanState;
  startScan: (path?: string) => Promise<void>;
  cancelScan: () => Promise<void>;
  refreshBackends: () => Promise<void>;
  chooseBackend: (id: string | null) => Promise<void>;
}

const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: { children: ReactNode }) {
  const transport = useMemo(() => resolveTransport(), []);
  const [backends, setBackends] = useState<Backend[] | null>(null);
  const [selection, setSelection] = useState<BackendSelection | null>(null);
  const [backendError, setBackendError] = useState<string | null>(null);
  const [listenersReady, setListenersReady] = useState(false);
  const [scan, setScan] = useState<ScanState>({ status: "idle" });

  const refreshBackends = useCallback(async () => {
    setBackendError(null);
    try {
      const [detected, resolved, summary] = await Promise.all([
        transport.listBackends(),
        transport.backendSelection(),
        transport.scanSummary(),
      ]);
      setBackends(detected);
      setSelection(resolved);
      if (summary) setScan({ status: "done", summary });
    } catch (error) {
      setBackends([]);
      setBackendError(String(error));
    }
  }, [transport]);

  useEffect(() => void refreshBackends(), [refreshBackends]);

  useEffect(() => {
    let live = true;
    const off: Unsubscribe[] = [];
    const keep = async (pending: Promise<Unsubscribe>) => {
      const unsubscribe = await pending;
      if (live) off.push(unsubscribe);
      else unsubscribe();
    };

    Promise.all([
      keep(
        transport.onScanProgress((progress) =>
          setScan((current) =>
            current.status === "scanning" ? { ...current, progress } : current,
          ),
        ),
      ),
      keep(transport.onScanFinished((summary) => setScan({ status: "done", summary }))),
      keep(
        transport.onScanFailed((failure) =>
          setScan(
            failure.cancelled
              ? { status: "cancelled" }
              : { status: "failed", message: failure.message },
          ),
        ),
      ),
    ]).then(
      () => live && setListenersReady(true),
      (error: unknown) => {
        if (live) {
          setBackendError(`Could not subscribe to scan events: ${String(error)}`);
          setListenersReady(false);
        }
      },
    );

    return () => {
      live = false;
      off.forEach((unsubscribe) => unsubscribe());
    };
  }, [transport]);

  const startScan = useCallback(
    async (path = "~") => {
      const requested = path.trim() || "~";
      setScan({
        status: "scanning",
        root: requested,
        progress: { scanned: 0, currentPath: requested },
      });
      try {
        const root = await transport.startScan(requested);
        setScan((current) =>
          current.status === "scanning"
            ? { ...current, root, progress: { ...current.progress, currentPath: root } }
            : current,
        );
      } catch (error) {
        setScan({ status: "failed", message: String(error) });
      }
    },
    [transport],
  );

  const cancelScan = useCallback(async () => {
    await transport.cancelScan();
  }, [transport]);

  const chooseBackend = useCallback(
    async (id: string | null) => {
      setSelection(await transport.chooseBackend(id));
      await refreshBackends();
    },
    [refreshBackends, transport],
  );

  return (
    <AppContext.Provider
      value={{
        transport,
        isShell: isTauri(),
        backends,
        selection,
        backendError,
        listenersReady,
        scan,
        startScan,
        cancelScan,
        refreshBackends,
        chooseBackend,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export function useApp() {
  const value = useContext(AppContext);
  if (!value) throw new Error("useApp must be used inside AppProvider");
  return value;
}
