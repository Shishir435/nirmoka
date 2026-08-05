/**
 * The one dialog that stands between a click and a file moving.
 *
 * Shared by the browser and the applications list rather than written twice.
 * Two copies of a destructive confirmation drift, and the copy that drifts is
 * the one nobody was looking at.
 *
 * Everything it shows comes from Rust: the path is the *resolved* one, checked
 * by the validator, not the name of the row that was clicked, and the sentence
 * is the one the preparation carried. A dialog that describes the click rather
 * than the check is a dialog that can be truthful about the wrong file.
 */

import type { TrashPreparation } from "@nirmoka/transport";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { formatBytes } from "@/lib/format";

export function TrashConfirmation({
  preparation,
  /** The platform's own wording, from `platformFeatures`. */
  label,
  onCancel,
  onConfirm,
}: {
  preparation: TrashPreparation | null;
  label: string;
  onCancel: () => void;
  onConfirm: (confirmationToken: number) => void;
}) {
  return (
    <Dialog open={preparation !== null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent>
        {preparation && (
          <>
            <DialogHeader>
              <DialogTitle>
                {label}
                {preparation.isDirectory ? " this folder?" : " this item?"}
              </DialogTitle>
              <DialogDescription>{preparation.warning}</DialogDescription>
            </DialogHeader>
            <dl className="space-y-2 text-sm">
              <div className="flex justify-between gap-4">
                <dt className="text-muted-foreground shrink-0">Path</dt>
                <dd className="truncate text-right font-mono text-xs">{preparation.targetPath}</dd>
              </div>
              <div className="flex justify-between gap-4">
                <dt className="text-muted-foreground shrink-0">Size</dt>
                <dd className="text-right font-mono">{formatBytes(preparation.totalBytes)}</dd>
              </div>
            </dl>
            <div className="mt-6 flex justify-end gap-2">
              <Button variant="outline" onClick={onCancel}>
                Cancel
              </Button>
              <Button onClick={() => onConfirm(preparation.confirmationToken)}>{label}</Button>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
