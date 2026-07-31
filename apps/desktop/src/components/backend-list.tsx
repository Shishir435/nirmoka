import type { Backend, Detection } from "@nirmoka/transport";

import { Badge } from "@/components/ui/badge";

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

function State({ backend }: { backend: Backend }) {
  if (backend.usable && backend.detection?.state === "found") {
    return <Badge variant="secondary">{backend.detection.version}</Badge>;
  }

  if (backend.detection?.state === "unsupportedVersion") {
    return <Badge variant="destructive">unsupported</Badge>;
  }

  return <Badge variant="outline">missing</Badge>;
}

export function BackendList({ backends }: { backends: Backend[] | null }) {
  if (backends === null) {
    return <p className="text-muted-foreground text-sm">Detecting…</p>;
  }

  if (backends.length === 0) {
    return <p className="text-muted-foreground text-sm">No backends are registered.</p>;
  }

  return (
    <ul className="divide-border divide-y overflow-hidden rounded-lg border">
      {backends.map((backend) => (
        <li key={backend.id} className="flex items-baseline justify-between gap-4 px-4 py-3">
          <div className="min-w-0">
            <p className="text-sm font-medium">{backend.displayName}</p>
            <p className="text-muted-foreground truncate font-mono text-xs">
              {describe(backend.detection, backend.error)}
            </p>
          </div>
          <State backend={backend} />
        </li>
      ))}
    </ul>
  );
}
