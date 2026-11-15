import { useState } from "react";
import { motion } from "motion/react";
import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { ArrowSquareOutIcon, HardDrivesIcon, LockSimpleIcon, PlusIcon, StarIcon } from "@phosphor-icons/react";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { InteractiveCard } from "@hearth/ui/components/card";
import { Pressable } from "@hearth/ui/components/pressable";
import { Stars } from "@hearth/ui/components/stars";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { formatCompact } from "@hearth/ui/format";
import { riseIn } from "@hearth/ui/motion";
import { useUpdates } from "../../api/hooks";
import type { CatalogApp } from "../../api/types";
import { AppSheet } from "./app-sheet";
import { FitBadge } from "./fit-badge";
import { InstallDialog } from "./install-dialog";
import { RemoveDialog } from "./remove-dialog";

type DialogKind = "sheet" | "install" | "remove" | null;

/** Hosts the three app dialogs so every app surface (card, row, hero) behaves the same. */
export function useAppDialogs(app: CatalogApp) {
  const [dialog, setDialog] = useState<DialogKind>(null);
  const close = (o: boolean) => {
    if (!o) setDialog(null);
  };
  const node = (
    <>
      {dialog === "sheet" ? (
        <AppSheet app={app} open onOpenChange={close} onInstall={() => setDialog("install")} onRemove={() => setDialog("remove")} />
      ) : null}
      {dialog === "install" ? <InstallDialog app={app} open onOpenChange={close} /> : null}
      {dialog === "remove" ? <RemoveDialog app={app} open onOpenChange={close} /> : null}
    </>
  );
  return { open: setDialog, node };
}

export function AppCard({ app, index }: { app: CatalogApp; index: number }) {
  const dialogs = useAppDialogs(app);
  const updates = useUpdates();
  const hasUpdate = updates.data?.apps[app.id]?.available ?? false;
  return (
    <motion.div layout variants={riseIn} custom={index} initial="hidden" animate="show" exit="exit">
      <InteractiveCard className="flex h-full flex-col gap-3">
        <button type="button" onClick={() => dialogs.open("sheet")} className="flex items-start gap-3 rounded-lg text-left" aria-label={`${app.name} details`}>
          <AppLogo src={app.icon} name={app.name} category={app.category} />
          <span className="min-w-0 flex-1">
            <span className="block truncate font-semibold text-kumo-default">{app.name}</span>
            <span className="flex items-center gap-2 text-xs text-kumo-subtle">
              <span className="capitalize">{app.category === "ai" ? "AI" : app.category}</span>
              {app.stats?.stars != null ? (
                <span className="inline-flex items-center gap-0.5 tabular-nums">
                  <StarIcon size={11} weight="fill" aria-hidden="true" /> {formatCompact(app.stats.stars)}
                </span>
              ) : null}
            </span>
          </span>
          {app.installed ? (
            <span className="flex items-center gap-1.5 text-xs text-kumo-subtle">
              <StatusDot tone={app.running ? "success" : "neutral"} live={app.running} label={app.running ? "Running" : "Stopped"} />
              {app.running ? "Running" : "Stopped"}
            </span>
          ) : app.verdict ? (
            <FitBadge fit={app.verdict.fit} />
          ) : null}
        </button>

        <p className="line-clamp-2 min-h-10 text-sm text-kumo-strong text-pretty">{app.description}</p>

        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-kumo-subtle">
          <span className="tabular-nums">~{app.ram_mb}&nbsp;MB RAM</span>
          {app.needs_vault ? (
            <span className="inline-flex items-center gap-1">
              <HardDrivesIcon size={12} aria-hidden="true" /> Uses SSD
            </span>
          ) : null}
          {app.protected ? (
            <span className="inline-flex items-center gap-1">
              <LockSimpleIcon size={12} aria-hidden="true" /> Login
            </span>
          ) : null}
          {app.rating ? <Stars value={app.rating} size={11} /> : null}
          {app.installed && hasUpdate ? (
            <button type="button" onClick={() => dialogs.open("sheet")} className="font-medium text-kumo-link hover:underline">
              Update available
            </button>
          ) : null}
        </div>

        <div className="mt-auto flex items-center gap-2 pt-1">
          {app.installed ? (
            <Pressable>
              <LinkButton href={app.url} external variant="primary" size="sm" icon={<ArrowSquareOutIcon />}>
                Open
              </LinkButton>
            </Pressable>
          ) : (
            <Pressable>
              <Button variant={app.verdict?.fit === "deny" ? "secondary" : "primary"} size="sm" icon={<PlusIcon />} onClick={() => dialogs.open("install")}>
                Install
              </Button>
            </Pressable>
          )}
          <Button variant="ghost" size="sm" onClick={() => dialogs.open("sheet")}>
            Details
          </Button>
        </div>
      </InteractiveCard>
      {dialogs.node}
    </motion.div>
  );
}

/** Compact ranked row for leaderboards ("1  [logo] Name  …  Get"). */
export function AppRow({ app, rank, detail }: { app: CatalogApp; rank?: number; detail?: string }) {
  const dialogs = useAppDialogs(app);
  return (
    <li className="flex items-center gap-3 py-2">
      {rank != null ? <span className="w-6 text-right text-lg font-semibold tabular-nums text-kumo-subtle">{rank}</span> : null}
      <button type="button" onClick={() => dialogs.open("sheet")} className="flex min-w-0 flex-1 items-center gap-3 rounded-lg text-left">
        <AppLogo src={app.icon} name={app.name} category={app.category} size="sm" />
        <span className="min-w-0">
          <span className="block truncate text-sm font-medium">{app.name}</span>
          <span className="block truncate text-xs text-kumo-subtle">{detail ?? app.description}</span>
        </span>
      </button>
      {app.installed ? (
        <LinkButton href={app.url} external size="xs" variant="secondary">
          Open
        </LinkButton>
      ) : (
        <Button size="xs" variant="secondary" onClick={() => dialogs.open("install")}>
          Get
        </Button>
      )}
      {dialogs.node}
    </li>
  );
}
