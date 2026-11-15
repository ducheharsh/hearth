import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Loader } from "@cloudflare/kumo/components/loader";
import { Switch } from "@cloudflare/kumo/components/switch";
import {
  BrainIcon,
  CheckCircleIcon,
  CircleIcon,
  CodeIcon,
  CpuIcon,
  FilmStripIcon,
  FolderSimpleIcon,
  HardDrivesIcon,
  PackageIcon,
  ShieldCheckIcon,
  UsersThreeIcon,
  WarningCircleIcon,
  XIcon,
  type Icon,
} from "@phosphor-icons/react";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { InteractiveCard } from "@hearth/ui/components/card";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { Section } from "@hearth/ui/components/section";
import { riseIn } from "@hearth/ui/motion";
import { useCatalog, useInstallPack, usePackPlan, usePacks, type PackInfo } from "../../api/hooks";
import { FitBadge } from "../marketplace/fit-badge";

const ICONS: Record<string, Icon> = {
  brain: BrainIcon,
  "users-three": UsersThreeIcon,
  "film-strip": FilmStripIcon,
  "shield-check": ShieldCheckIcon,
  code: CodeIcon,
  "hard-drives": HardDrivesIcon,
  "folder-simple": FolderSimpleIcon,
};

const gb = (mb: number) => (mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`);

/** Store row: one card per pack. */
export function Packs() {
  const { data } = usePacks();
  const [open, setOpen] = useState<string | null>(null);
  if (!data?.length) return null;
  return (
    <Section title="Packs" description="Set up a whole use case in one go: every app it needs, checked against this Mac’s capacity.">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {data.map((p, i) => (
          <PackCard key={p.pack.id} info={p} index={i} onOpen={() => setOpen(p.pack.id)} />
        ))}
      </div>
      {open ? <PackDialog id={open} onClose={() => setOpen(null)} /> : null}
    </Section>
  );
}

function PackCard({ info, index, onOpen }: { info: PackInfo; index: number; onOpen: () => void }) {
  const { pack, plan, job } = info;
  const I = ICONS[pack.icon] ?? PackageIcon;
  const running = job != null && job.finished == null;
  const required = plan.apps.filter((a) => !a.optional);
  return (
    <motion.div variants={riseIn} custom={index} initial="hidden" animate="show">
      <InteractiveCard className="h-full p-0">
        <button type="button" onClick={onOpen} className="flex h-full w-full flex-col gap-3 rounded-xl p-5 text-left" aria-label={`${pack.name} pack`}>
          <span className="flex items-start justify-between gap-3">
            <span className="grid size-11 shrink-0 place-items-center rounded-xl bg-kumo-tint text-kumo-brand">
              <I size={24} weight="duotone" aria-hidden="true" />
            </span>
            {running ? (
              <Badge variant="info" appearance="dot">Installing…</Badge>
            ) : plan.complete ? (
              <Badge variant="success" appearance="dot">Installed</Badge>
            ) : null}
          </span>
          <span className="min-w-0">
            <span className="block font-semibold">{pack.name}</span>
            <span className="block text-sm text-kumo-subtle text-pretty">{pack.tagline}</span>
          </span>
          <span className="mt-auto flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-kumo-subtle">
            <span>
              {required.length} app{required.length === 1 ? "" : "s"}
              {plan.models.length ? ` + ${plan.models.filter((m) => !m.optional).length} AI models` : ""}
            </span>
            {plan.added.ram_mb > 0 ? <span className="tabular-nums">+{gb(plan.added.ram_mb)} RAM</span> : null}
          </span>
        </button>
      </InteractiveCard>
    </motion.div>
  );
}

function PackDialog({ id, onClose }: { id: string; onClose: () => void }) {
  const [withIds, setWith] = useState<string[]>([]);
  const { data, isFetching } = usePackPlan(id, withIds);
  const live = usePacks().data?.find((p) => p.pack.id === id);
  const catalog = useCatalog();
  const install = useInstallPack();
  const job = live?.job ?? null;
  const running = job != null && job.finished == null;
  const toggle = (k: string, on: boolean) => setWith((w) => (on ? [...w, k] : w.filter((x) => x !== k)));
  const app = (aid: string) => catalog.data?.apps.find((a) => a.id === aid);

  const info = data;
  const I = ICONS[info?.pack.icon ?? ""] ?? PackageIcon;
  const deny = info?.verdict?.fit === "deny";
  const nothing = info?.plan.complete ?? false;
  const failed = job?.steps.filter((s) => s.state === "failed").length ?? 0;

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="lg" className={DIALOG_FRAME}>
        <DialogFrame
          header={
            <div className="flex items-start justify-between gap-3">
              <div className="flex min-w-0 items-start gap-3">
                <span className="grid size-11 shrink-0 place-items-center rounded-xl bg-kumo-tint text-kumo-brand">
                  <I size={24} weight="duotone" aria-hidden="true" />
                </span>
                <div className="min-w-0">
                  <Dialog.Title className="text-lg font-semibold">{info?.pack.name ?? "Pack"}</Dialog.Title>
                  <Dialog.Description className="text-sm text-kumo-subtle text-pretty">{info?.pack.tagline}</Dialog.Description>
                </div>
              </div>
              <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
            </div>
          }
          footer={
            job ? (
              <Button variant="primary" onClick={onClose}>
                {running ? "Keep Installing in Background" : "Done"}
              </Button>
            ) : (
              <>
                <Dialog.Close render={(p) => <Button {...p} variant="secondary">Cancel</Button>} />
                <Button
                  variant="primary"
                  disabled={!info || nothing}
                  loading={install.isPending}
                  onClick={() => install.mutate({ id, with: withIds, force: deny })}
                >
                  {nothing ? "Everything’s Installed" : deny ? "Install Anyway" : "Install Pack"}
                </Button>
              </>
            )
          }
        >
          {!info ? (
            <div className="flex items-center gap-2 text-sm text-kumo-subtle">
              <Loader size={16} /> Checking this Mac…
            </div>
          ) : job ? (
            <JobView info={info} job={job} failed={failed} />
          ) : (
            <div className="flex flex-col gap-6">
              <p className="text-sm text-kumo-strong text-pretty">{info.pack.description}</p>

              <section aria-label="Apps" className="flex flex-col gap-1">
                <h3 className="text-xs font-semibold uppercase tracking-wider text-kumo-subtle">Apps</h3>
                <ul className="divide-y divide-kumo-hairline">
                  {info.plan.apps.map((a) => {
                    const c = app(a.id);
                    return (
                      <li key={a.id} className="flex items-center gap-3 py-2.5">
                        <AppLogo src={c?.icon} name={a.name} category={c?.category ?? "other"} size="sm" />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-sm font-medium">{a.name}</span>
                          <span className="block text-xs text-kumo-subtle text-pretty">{a.role} · ~{a.ram_mb} MB</span>
                        </span>
                        <Member state={a.state} optional={a.optional} label={a.name} onToggle={(on) => toggle(a.id, on)} />
                      </li>
                    );
                  })}
                </ul>
              </section>

              {info.plan.models.length ? (
                <section aria-label="AI models" className="flex flex-col gap-1">
                  <h3 className="text-xs font-semibold uppercase tracking-wider text-kumo-subtle">AI Models</h3>
                  <p className="text-xs text-kumo-subtle text-pretty">
                    {info.ollama.running
                      ? `Ollama ${info.ollama.version ?? ""} is running natively.`
                      : info.ollama.installed
                        ? "Ollama is installed; hearth will start it."
                        : "hearth installs Ollama natively with Homebrew so models run on the M1’s GPU."}{" "}
                    Only one model is loaded at a time.
                  </p>
                  <ul className="divide-y divide-kumo-hairline">
                    {info.plan.models.map((m) => (
                      <li key={m.name} className="flex items-center gap-3 py-2.5">
                        <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-kumo-tint text-kumo-subtle">
                          <CpuIcon size={16} aria-hidden="true" />
                        </span>
                        <span className="min-w-0 flex-1">
                          <span className="block truncate font-mono text-sm">{m.name}</span>
                          <span className="block text-xs text-kumo-subtle text-pretty">
                            {m.role} · {gb(m.size_mb)} download · {gb(m.ram_mb)} when loaded
                          </span>
                        </span>
                        <Member state={m.state} optional={m.optional} label={m.name} onToggle={(on) => toggle(m.name, on)} />
                      </li>
                    ))}
                  </ul>
                </section>
              ) : null}

              {!nothing && info.verdict ? (
                <section aria-label="Capacity" className="flex flex-col gap-2 rounded-xl p-4 ring-1 ring-kumo-hairline">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="text-sm font-medium">
                      Adds ~{gb(info.plan.added.ram_mb)} RAM{info.plan.download_mb ? ` · ${gb(info.plan.download_mb)} of models` : ""}
                    </span>
                    {isFetching ? <Loader size={14} /> : <FitBadge fit={info.verdict.fit} />}
                  </div>
                  <ul className="flex flex-col gap-1 text-xs text-kumo-subtle">
                    {info.verdict.reasons.map((r) => (
                      <li key={r}>{r}</li>
                    ))}
                  </ul>
                </section>
              ) : null}
              {install.error ? <Banner variant="error" title="Couldn’t start" description={install.error.message} /> : null}
            </div>
          )}
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}

function Member({ state, optional, label, onToggle }: { state: string; optional: boolean; label: string; onToggle: (on: boolean) => void }) {
  if (state === "installed") return <Badge variant="success" appearance="dot">Installed</Badge>;
  if (!optional) return <span className="text-xs text-kumo-subtle">Included</span>;
  return <Switch size="sm" checked={state === "install"} onCheckedChange={onToggle} aria-label={`Include ${label}`} />;
}

function JobView({ info, job, failed }: { info: PackInfo; job: NonNullable<PackInfo["job"]>; failed: number }) {
  const finished = job.finished != null;
  return (
    <div className="flex flex-col gap-5">
      <ol className="flex flex-col gap-3" aria-live="polite">
        {job.steps.map((s) => (
          <li key={s.label} className="flex flex-col gap-1.5">
            <span className="flex items-center gap-2.5 text-sm">
              {s.state === "done" ? (
                <CheckCircleIcon size={18} weight="fill" className="text-kumo-success" aria-hidden="true" />
              ) : s.state === "failed" ? (
                <WarningCircleIcon size={18} weight="fill" className="text-kumo-danger" aria-hidden="true" />
              ) : s.state === "running" ? (
                <Loader size={18} />
              ) : (
                <CircleIcon size={18} className="text-kumo-subtle" aria-hidden="true" />
              )}
              <span className={s.state === "pending" ? "text-kumo-subtle" : ""}>{s.label}</span>
              {s.state === "running" && s.progress != null ? <span className="ml-auto text-xs tabular-nums text-kumo-subtle">{s.progress}%</span> : null}
            </span>
            {s.state === "running" && s.progress != null ? (
              <span className="ml-7 h-1 overflow-hidden rounded-full bg-kumo-tint">
                <motion.span className="block h-full bg-kumo-brand" animate={{ width: `${s.progress}%` }} transition={{ duration: 0.3 }} />
              </span>
            ) : null}
            {s.error ? <p className="ml-7 text-xs text-kumo-danger text-pretty">{s.error}</p> : null}
          </li>
        ))}
      </ol>
      <AnimatePresence>
        {finished ? (
          <motion.div initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }}>
            {failed ? (
              <Banner variant="error" title={`${failed} step${failed > 1 ? "s" : ""} failed`} description="Fix the issue above, then open the pack again to install what’s missing." />
            ) : (
              <div className="flex flex-col gap-2 rounded-xl p-4 ring-1 ring-kumo-hairline">
                <p className="text-sm font-medium">{info.pack.name} is ready.</p>
                {info.pack.next_steps.length ? (
                  <ul className="flex list-disc flex-col gap-1 pl-5 text-sm text-kumo-subtle">
                    {info.pack.next_steps.map((n) => (
                      <li key={n}>{n}</li>
                    ))}
                  </ul>
                ) : null}
              </div>
            )}
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}
