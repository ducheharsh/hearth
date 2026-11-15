import { useDeferredValue, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Checkbox } from "@cloudflare/kumo/components/checkbox";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Loader } from "@cloudflare/kumo/components/loader";
import { useKumoToastManager } from "@cloudflare/kumo/components/toast";
import { DownloadSimpleIcon, LightningIcon, SlidersHorizontalIcon, WarningIcon, XIcon } from "@phosphor-icons/react";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { Pressable } from "@hearth/ui/components/pressable";
import { UsageBar } from "@hearth/ui/components/usage-bar";
import { formatBytes } from "@hearth/ui/format";
import { springSoft } from "@hearth/ui/motion";
import { useAppConfig, useDryRun, useInstall, useSystem } from "../../api/hooks";
import type { AppConfig, CatalogApp } from "../../api/types";
import { ConfigForm } from "./config-form";
import { FitBadge } from "./fit-badge";

interface InstallDialogProps {
  app: CatalogApp;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Recommended one-click install, or customize every Docker setting first. */
export function InstallDialog({ app, open, onOpenChange }: InstallDialogProps) {
  const [customize, setCustomize] = useState(false);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [override, setOverride] = useState(false);
  const deferredConfig = useDeferredValue(config);
  const cfg = useAppConfig(app.id, open && customize);
  const dry = useDryRun(app.id, deferredConfig, open);
  const system = useSystem();
  const install = useInstall();
  const toasts = useKumoToastManager();

  const verdict = dry.data?.verdict;
  const s = system.data?.latest;
  const denied = verdict?.fit === "deny";

  const run = () =>
    install.mutate(
      { id: app.id, force: denied && override, config },
      {
        onSuccess: () => onOpenChange(false),
        onError: (e) =>
          toasts.add({ variant: "error", title: `Couldn’t Install ${app.name}`, description: `${e.message}. Check Docker is running, then try again.` }),
      },
    );

  return (
    <Dialog.Root open={open} onOpenChange={(o) => !install.isPending && onOpenChange(o)}>
      <Dialog size={customize ? "xl" : "lg"} className={DIALOG_FRAME}>
        <DialogFrame
          header={
        <div className="flex items-start gap-3">
          <AppLogo src={app.icon} name={app.name} category={app.category} />
          <div className="min-w-0 flex-1">
            <Dialog.Title className="text-lg font-semibold">Install {app.name}</Dialog.Title>
            <Dialog.Description className="text-sm text-kumo-subtle">{app.description}</Dialog.Description>
          </div>
          <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
        </div>
          }
          footer={
            <>
              <Dialog.Close render={(p) => <Button {...p} variant="secondary">Cancel</Button>} />
              <Pressable>
                <Button
                  variant={denied ? "destructive" : "primary"}
                  icon={denied ? <LightningIcon /> : <DownloadSimpleIcon />}
                  loading={install.isPending}
                  disabled={!verdict || Boolean(dry.error) || (denied && !override)}
                  onClick={run}
                >
                  {install.isPending ? "Installing…" : denied ? "Force Install" : config ? "Install with My Settings" : `Install ${app.name}`}
                </Button>
              </Pressable>
            </>
          }
        >
        <div className="flex flex-col gap-5">
        <AnimatePresence mode="wait" initial={false}>
          {dry.isPending ? (
            <motion.div key="loading" className="flex items-center gap-2 text-sm text-kumo-subtle" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} aria-live="polite">
              <Loader size={16} /> Checking capacity…
            </motion.div>
          ) : dry.error ? (
            <motion.div key="error" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
              <Banner variant="error" icon={<WarningIcon />} title="These settings don’t work" description={dry.error.message} />
            </motion.div>
          ) : verdict ? (
            <motion.div key="verdict" className="flex flex-col gap-3" initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={springSoft} aria-live="polite">
              <div className="flex items-center justify-between gap-3">
                <span className="text-sm font-medium">Capacity Check</span>
                <FitBadge fit={verdict.fit} />
              </div>
              {s ? (
                <UsageBar
                  label="Memory after install"
                  value={s.mem_used}
                  projected={s.mem_used + app.ram_mb * 1024 * 1024}
                  max={s.mem_total}
                  detail={`~${formatBytes(Math.max(verdict.mem_after, 0))} free afterwards`}
                />
              ) : null}
              <ul className="flex flex-col gap-1 text-sm">
                {verdict.reasons.map((r) => (
                  <li key={r} className="flex gap-2">
                    <span aria-hidden="true" className={verdict.fit === "deny" ? "text-kumo-danger" : verdict.fit === "warn" ? "text-kumo-warning" : "text-kumo-success"}>
                      •
                    </span>
                    <span className="min-w-0 text-pretty">{r}</span>
                  </li>
                ))}
              </ul>
            </motion.div>
          ) : null}
        </AnimatePresence>

        <section className="flex flex-col gap-3" aria-label="Docker settings">
          <button
            type="button"
            onClick={() => setCustomize((c) => !c)}
            aria-expanded={customize}
            className="flex items-center gap-2 self-start rounded-md text-sm font-medium text-kumo-link hover:underline"
          >
            <SlidersHorizontalIcon size={16} aria-hidden="true" />
            {customize ? "Hide Docker settings" : "Customize Docker settings"}
            {config ? <span className="text-xs text-kumo-warning">(customized)</span> : null}
          </button>
          <AnimatePresence initial={false}>
            {customize ? (
              <motion.div key="form" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} className="overflow-hidden">
                {cfg.data ? (
                  <ConfigForm data={cfg.data} value={config} onChange={setConfig} />
                ) : (
                  <div className="flex items-center gap-2 text-sm text-kumo-subtle">
                    <Loader size={14} /> Loading settings…
                  </div>
                )}
              </motion.div>
            ) : null}
          </AnimatePresence>
        </section>

        {denied ? (
          <Checkbox label="Install anyway. I understand this may make the machine swap or run out of disk." checked={override} onCheckedChange={(v) => setOverride(v === true)} />
        ) : null}

        </div>
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}
