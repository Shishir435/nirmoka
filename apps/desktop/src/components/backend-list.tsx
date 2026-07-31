import type { Backend, BackendSelection, Detection } from "@nirmoka/transport";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

/**
 * What detection found, said plainly.
 *
 * `unsupportedVersion` gets its own line rather than being folded into "not
 * installed": telling someone to install what they already have is worse than
 * saying nothing.
 */
function describe(detection: Detection | null, error: string | null): string {
  if (error) return error;

  switch (detection?.state) {
    case "found":
      return detection.path;
    case "unsupportedVersion":
      return `found ${detection.version} at ${detection.path}, this build understands ${detection.supported}`;
    case "notInstalled":
      return "not on PATH";
    default:
      return "detection did not run";
  }
}

/**
 * What this backend is for, in the words of its own capability flags.
 *
 * Without it, a usable Mole sits in the list next to a usable ncdu looking
 * identical, while only one of them can answer the button above. Naming the
 * abilities is how "degrade, don't lie" reaches the screen — the alternative is
 * a user concluding the app ignores a backend it has detected.
 */
function Abilities({ backend }: { backend: Backend }) {
  if (!backend.usable) return null;

  const abilities = [
    backend.capabilities.scan && "scan",
    backend.capabilities.cleanupCategories && "clean",
    backend.capabilities.uninstallApps && "uninstall",
    backend.capabilities.systemStatus && "status",
  ].filter(Boolean);

  if (abilities.length === 0) return null;

  return <p className="text-muted-foreground mt-0.5 text-xs">{abilities.join(" · ")}</p>;
}

function State({ backend }: { backend: Backend }) {
  if (backend.usable && backend.detection?.state === "found") {
    return <Badge variant="secondary">{backend.detection.version}</Badge>;
  }

  if (backend.detection?.state === "unsupportedVersion") {
    return <Badge variant="destructive">unsupported</Badge>;
  }

  return <Badge variant="outline">missing</Badge>;
}

/**
 * Which backend is preferred, and what that actually resolves to.
 *
 * The second sentence is the one that matters. A preference is honoured
 * wherever the backend can do the job and fallen back from where it cannot, so
 * "Mole is preferred" and "ncdu scanned this" are both true at once on macOS.
 * Showing only the first would make the app look like it ignores the setting;
 * showing only the second would make the setting look like it did nothing.
 */
function Resolution({ selection, backends }: { selection: BackendSelection; backends: Backend[] }) {
  const find = (id: string) => backends.find((backend) => backend.id === id);
  const nameOf = (id: string) => find(id)?.displayName ?? id;

  /**
   * Why a preferred backend is not the one running.
   *
   * Two different reasons with two different fixes, and guessing between them
   * is how a user gets told to install what they already have. The Rust side
   * deliberately does not claim one — it reports only that the choice was not
   * met — so the answer is read here, from the detection the list already has.
   */
  const why = (id: string) => {
    const backend = find(id);
    if (!backend) return "is not a backend this build knows";
    if (backend.detection?.state === "unsupportedVersion") return "is at an untested version";
    if (!backend.usable) return "is not installed";
    if (!backend.capabilities.scan) return "cannot scan";
    return "is not available for this";
  };

  if (selection.scanner === null) {
    return (
      <p className="text-muted-foreground text-xs">
        Nothing installed can scan. Install ncdu to browse a disk.
      </p>
    );
  }

  return (
    <div className="text-muted-foreground space-y-1 text-xs">
      <p>
        Scans run on{" "}
        <span className="text-foreground font-medium">{nameOf(selection.scanner)}</span>
        {selection.scannerInsteadOf !== null && (
          <>
            {" "}
            — {nameOf(selection.scannerInsteadOf)} is preferred and{" "}
            {why(selection.scannerInsteadOf)}
          </>
        )}
        .
      </p>
      {!selection.persistent && (
        <p>This machine has no configuration directory, so the choice lasts until you quit.</p>
      )}
      {/* The choice took effect; only writing it down failed. Saying both is
          the difference between a setting that did not apply and one that will
          not survive a restart — the second needs no retry, just a warning. */}
      {selection.saveError !== null && (
        <p className="text-destructive">
          Applied, but not saved — {selection.saveError}. It will go back to the default when you
          quit.
        </p>
      )}
    </div>
  );
}

/**
 * The picker.
 *
 * Only rendered with two or more usable backends: a single-backend machine has
 * no choice to make, and a radio group with one option is a control that exists
 * to be ignored. "Automatic" stays available as a real option rather than as the
 * absence of one, because it is not the same as picking whichever backend
 * happens to be first today — it keeps following the platform default when a
 * later release changes it.
 */
function Picker({
  backends,
  selection,
  onChoose,
  busy,
}: {
  backends: Backend[];
  selection: BackendSelection;
  onChoose: (id: string | null) => void;
  busy: boolean;
}) {
  const usable = backends.filter((backend) => backend.usable);
  if (usable.length < 2) return null;

  // What "Automatic" resolves to right now: the first usable backend the
  // platform default names. Naming it turns an opaque option into a visible one.
  const automatic = selection.defaultOrder
    .map((id) => usable.find((backend) => backend.id === id))
    .find((backend) => backend !== undefined);

  const options: { id: string | null; label: string }[] = [
    { id: null, label: automatic ? `Automatic (${automatic.displayName})` : "Automatic" },
    ...usable.map((backend) => ({ id: backend.id, label: backend.displayName })),
  ];

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-muted-foreground text-xs">Preferred backend</span>
      {options.map((option) => (
        <Button
          key={option.id ?? "automatic"}
          size="sm"
          variant={selection.chosen === option.id ? "default" : "outline"}
          disabled={busy}
          onClick={() => onChoose(option.id)}
        >
          {option.label}
        </Button>
      ))}
    </div>
  );
}

export function BackendList({
  backends,
  selection,
  onChoose,
  busy = false,
}: {
  backends: Backend[] | null;
  selection: BackendSelection | null;
  onChoose: (id: string | null) => void;
  busy?: boolean;
}) {
  if (backends === null) {
    return <p className="text-muted-foreground text-sm">Detecting…</p>;
  }

  if (backends.length === 0) {
    return <p className="text-muted-foreground text-sm">No backends are registered.</p>;
  }

  return (
    <div className="space-y-3">
      <ul className="divide-border divide-y overflow-hidden rounded-lg border">
        {backends.map((backend) => (
          <li key={backend.id} className="flex items-baseline justify-between gap-4 px-4 py-3">
            <div className="min-w-0">
              <p className="text-sm font-medium">
                {backend.displayName}
                {selection?.chosen === backend.id && (
                  <span className="text-muted-foreground ml-2 text-xs font-normal">preferred</span>
                )}
              </p>
              <p className="text-muted-foreground truncate font-mono text-xs">
                {describe(backend.detection, backend.error)}
              </p>
              <Abilities backend={backend} />
            </div>
            <State backend={backend} />
          </li>
        ))}
      </ul>

      {selection && (
        <>
          <Picker backends={backends} selection={selection} onChoose={onChoose} busy={busy} />
          <Resolution selection={selection} backends={backends} />
        </>
      )}
    </div>
  );
}
