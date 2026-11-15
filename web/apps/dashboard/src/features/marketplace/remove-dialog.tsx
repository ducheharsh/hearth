import { Button } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { useKumoToastManager } from "@cloudflare/kumo/components/toast";
import { TrashIcon } from "@phosphor-icons/react";
import { useRemove } from "../../api/hooks";
import type { CatalogApp } from "../../api/types";

interface RemoveDialogProps {
  app: CatalogApp;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Destructive actions always confirm. Data is kept, and the copy says so. */
export function RemoveDialog({ app, open, onOpenChange }: RemoveDialogProps) {
  const remove = useRemove();
  const toasts = useKumoToastManager();
  return (
    <Dialog.Root open={open} onOpenChange={(o) => !remove.isPending && onOpenChange(o)}>
      <Dialog size="base" className="flex flex-col gap-4 p-6">
        <Dialog.Title className="text-lg font-semibold">Remove {app.name}?</Dialog.Title>
        <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
          Containers stop and are deleted. Your data stays on disk, so reinstalling picks up where you left off.
        </Dialog.Description>
        <div className="flex justify-end gap-2">
          <Dialog.Close render={(p) => <Button {...p} variant="secondary">Keep It</Button>} />
          <Button
            variant="destructive"
            icon={<TrashIcon />}
            loading={remove.isPending}
            onClick={() =>
              remove.mutate(app.id, {
                onSuccess: (r) => {
                  onOpenChange(false);
                  toasts.add({ title: `${app.name} Removed`, description: `Data kept in ${r.data_kept_in}` });
                },
                onError: (e) =>
                  toasts.add({ variant: "error", title: `Couldn’t Remove ${app.name}`, description: e.message }),
              })
            }
          >
            {remove.isPending ? "Removing…" : "Remove App"}
          </Button>
        </div>
      </Dialog>
    </Dialog.Root>
  );
}
