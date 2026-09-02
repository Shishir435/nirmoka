/**
 * The plan a user reads before an application is removed.
 *
 * Everything here comes from the backend. The paths are the ones it said it would
 * touch, the sizes are its own rounded labels, and the sentence under the button
 * is the one the preparation carried. Nothing on this screen is Nirmoka's opinion
 * about what an uninstall involves — which matters, because this is the screen
 * that stands in for the backend's own confirmation prompt. See ADR 0027.
 *
 * Two things are deliberately hard to miss. Paths the backend says it will *not*
 * remove are shown as exactly that, because a list that mixed them in would
 * promise a cleanup that does not happen. And the raw transcript stays one click
 * away, so a parse that quietly dropped a row cannot become the whole story.
 */

import { AlertTriangle, ChevronRight, Trash2 } from "lucide-react";
import { useState } from "react";
import type { UninstallItem, UninstallPreparation, UninstallPreview } from "@nirmoka/transport";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { groupPlan, planCounts, survivingItems } from "@/lib/engine/uninstall-flow";

export function UninstallReview({
  preview,
  preparation,
  running,
  onCancel,
  /** Ask Rust for a token. Separate from confirming, so the token is issued
   *  against a plan the user has already been shown. */
  onApprove,
  onConfirm,
}: {
  preview: UninstallPreview | null;
  preparation: UninstallPreparation | null;
  running: boolean;
  onCancel: () => void;
  onApprove: () => void;
  onConfirm: (confirmationToken: number) => void;
}) {
  return (
    <Dialog open={preview !== null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent className="max-h-[85vh] gap-0 overflow-y-auto sm:max-w-2xl">
        {preview && (
          <ReviewBody {...{ preview, preparation, running, onCancel, onApprove, onConfirm }} />
        )}
      </DialogContent>
    </Dialog>
  );
}

function ReviewBody({
  preview,
  preparation,
  running,
  onCancel,
  onApprove,
  onConfirm,
}: {
  preview: UninstallPreview;
  preparation: UninstallPreparation | null;
  running: boolean;
  onCancel: () => void;
  onApprove: () => void;
  onConfirm: (confirmationToken: number) => void;
}) {
  const [showTranscript, setShowTranscript] = useState(false);
  const counts = planCounts(preview);
  const names = preview.apps.map((app) => app.name).join(", ");

  return (
    <>
      <DialogHeader>
        <DialogTitle>Uninstall {names}?</DialogTitle>
        <DialogDescription>
          {`${preview.backend} ${preview.backendVersion} found ${counts.removed} ${
            counts.removed === 1 ? "item" : "items"
          } to remove`}
          {preview.reportedTotal === null ? "" : `, about ${preview.reportedTotal}`}
          {counts.reviewOnly > 0
            ? `, and ${counts.reviewOnly} it will leave for you to review.`
            : "."}
        </DialogDescription>
      </DialogHeader>

      <div className="space-y-4 py-4">
        {preview.apps.map((app) => (
          <section key={app.name}>
            <h3 className="flex items-baseline gap-2 text-sm font-medium">
              {app.name}
              {app.homebrewCask && (
                <span className="rounded bg-muted px-1.5 py-0.5 text-xs font-normal text-muted-foreground">
                  Homebrew cask
                </span>
              )}
              {app.reportedSize !== null && (
                <span className="ml-auto shrink-0 font-mono text-xs tabular-nums text-muted-foreground">
                  {app.reportedSize}
                </span>
              )}
            </h3>
            {/* Grouped by where the paths sit, in the same vocabulary the
                Inspector uses for a footprint. A flat list of thirty paths is a
                transcript; these are the parts of the application. */}
            <div className="mt-2 space-y-3">
              {groupPlan(app.items).map((group) => (
                <div key={group.label}>
                  <p className="text-xs font-medium text-muted-foreground">{group.label}</p>
                  <ul className="mt-1 space-y-1">
                    {group.items.map((item, index) => (
                      <PlanRow key={`${item.displayPath}-${index}`} item={item} />
                    ))}
                  </ul>
                </div>
              ))}
            </div>

            {/* Where the approved design puts a "keep user data" choice. No
                flag backs that choice, so this is the true answer to the
                question it asks — quoted, not described. See ADR 0029. */}
            {survivingItems(app.items).length > 0 && (
              <div className="mt-3 rounded-md border bg-muted/40 p-3">
                <p className="text-xs font-medium">What this will not remove</p>
                <ul className="mt-1 space-y-1">
                  {survivingItems(app.items).map((item, index) => (
                    <li
                      key={`${item.displayPath}-${index}`}
                      className="truncate font-mono text-xs text-muted-foreground"
                      dir="ltr"
                    >
                      {item.displayPath}
                    </li>
                  ))}
                </ul>
                <p className="mt-2 text-xs text-muted-foreground">
                  {preview.backend} reported these and does not act on them. There is no option to
                  keep anything else: it removes what it decides to remove.
                </p>
              </div>
            )}
          </section>
        ))}

        {preview.warnings.length > 0 && (
          <ul className="space-y-1 rounded-md bg-muted p-3 text-xs text-muted-foreground">
            {preview.warnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        )}

        {/* The backend saying what it will not do. Worth more space than a
            warning, not less: it is the part a user has to act on themselves. */}
        {preview.notes.length > 0 && (
          <div className="rounded-md border border-warning/40 p-3">
            <p className="flex items-center gap-2 text-xs font-medium">
              <AlertTriangle className="size-3.5 shrink-0" />
              {preview.backend} will not handle these
            </p>
            <ul className="mt-1.5 space-y-1 text-xs text-muted-foreground">
              {preview.notes.map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          </div>
        )}

        <div>
          <button
            type="button"
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
            onClick={() => setShowTranscript((shown) => !shown)}
            aria-expanded={showTranscript}
          >
            <ChevronRight
              className={`size-3.5 transition-transform ${showTranscript ? "rotate-90" : ""}`}
            />
            {showTranscript ? "Hide" : "Show"} {preview.backend}&apos;s own output
          </button>
          {showTranscript && (
            <pre className="mt-2 max-h-64 overflow-auto rounded-md bg-muted p-3 font-mono text-[11px] leading-relaxed">
              {preview.transcript}
            </pre>
          )}
        </div>
      </div>

      {/* Two steps, and the second only appears once Rust has issued a token
          against this exact plan. A single button would mean approving a plan and
          asking for permission to run it in the same click, which is how a
          confirmation stops being one. */}
      {preparation === null ? (
        <div className="flex justify-end gap-2 border-t pt-4">
          <Button variant="outline" onClick={onCancel} disabled={running}>
            Cancel
          </Button>
          <Button onClick={onApprove} disabled={running}>
            Continue
          </Button>
        </div>
      ) : (
        <div className="space-y-3 border-t pt-4">
          <p className="text-sm">{preparation.warning}</p>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={onCancel} disabled={running}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              disabled={running}
              onClick={() => onConfirm(preparation.confirmationToken)}
            >
              <Trash2 />
              {running ? "Uninstalling…" : "Move to Trash"}
            </Button>
          </div>
        </div>
      )}
    </>
  );
}

function PlanRow({ item }: { item: UninstallItem }) {
  const reviewOnly = item.scope === "reviewOnly";
  return (
    <li className="flex items-baseline gap-2 text-xs">
      <span
        className={`shrink-0 ${reviewOnly ? "text-warning-foreground" : "text-muted-foreground"}`}
        aria-hidden
      >
        {reviewOnly ? "!" : "−"}
      </span>
      <span className={`min-w-0 flex-1 break-all font-mono ${reviewOnly ? "opacity-70" : ""}`}>
        {item.displayPath}
      </span>
      {item.scope === "system" && <span className="shrink-0 text-muted-foreground">system</span>}
      {reviewOnly && <span className="shrink-0 text-warning-foreground">left in place</span>}
      {item.reportedSize !== null && (
        <span className="shrink-0 font-mono tabular-nums text-muted-foreground">
          {item.reportedSize}
        </span>
      )}
    </li>
  );
}
