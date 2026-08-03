import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useState,
  type ReactNode,
} from "react";

import {
  isTauri,
  resolveTransport,
  type Backend,
  type BackendSelection,
  type Transport,
  type Unsubscribe,
} from "@nirmoka/transport";

import { INITIAL_SCAN, reduceScan, type ScanState } from "@/lib/engine/scan-machine";

export type { ScanState };

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
  // The transitions live in `scan-machine`, where the ones that are easy to get
  // wrong — late progress, cancellation, a rescan — are covered by tests.
  const [scan, dispatchScan] = useReducer(reduceScan, INITIAL_SCAN);

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
      if (summary) dispatchScan({ type: "restored", summary });
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
      keep(transport.onScanProgress((progress) => dispatchScan({ type: "progress", progress }))),
      keep(transport.onScanFinished((summary) => dispatchScan({ type: "finished", summary }))),
      keep(transport.onScanFailed((failure) => dispatchScan({ type: "failed", failure }))),
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
      dispatchScan({ type: "requested", root: requested });
      try {
        dispatchScan({ type: "rooted", root: await transport.startScan(requested) });
      } catch (error) {
        dispatchScan({ type: "failed", failure: { message: String(error), cancelled: false } });
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
