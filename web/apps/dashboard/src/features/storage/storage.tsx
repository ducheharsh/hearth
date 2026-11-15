import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Checkbox } from "@cloudflare/kumo/components/checkbox";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Empty } from "@cloudflare/kumo/components/empty";
import {
  ArchiveIcon,
  EjectIcon,
  FilmStripIcon,
  HardDrivesIcon,
  LinkBreakIcon,
  PlugsConnectedIcon,
  ShieldCheckIcon,
  VaultIcon,
  WarningIcon,
  XIcon,
} from "@phosphor-icons/react";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { Card, InteractiveCard } from "@hearth/ui/components/card";
import { Field, TextInput } from "@hearth/ui/components/form";
import { Section } from "@hearth/ui/components/section";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { UsageBar } from "@hearth/ui/components/usage-bar";
import { formatBytes, formatRelative } from "@hearth/ui/format";
import { riseIn } from "@hearth/ui/motion";
import { useAttach, useDetach, useEject, useStorage } from "../../api/hooks";
import type { AttachmentView, Drive, Role } from "../../api/types";

const ROLES: { role: Role; title: string; icon: typeof VaultIcon; blurb: string }[] = [
  { role: "vault", title: "Vault", icon: VaultIcon, blurb: "Photos, documents, synced files: app data that should live on the SSD." },
  { role: "media", title: "Media", icon: FilmStripIcon, blurb: "Movies, TV and music libraries for Jellyfin and friends." },
  { role: "backups", title: "Backups", icon: ArchiveIcon, blurb: "Backup repositories, kept on a different drive than the data." },
];

export function Storage() {
  const { data, error } = useStorage();
  const byRole = new Map(data?.attachments.map((a) => [a.role, a]));
  const unattached = data?.drives.filter((d) => !d.attached_as) ?? [];

  return (
    <div className="flex flex-col gap-10">
      {error ? <Banner variant="error" icon={<WarningIcon />} title="Can’t read drives" description={error.message} /> : null}
      {data?.error ? <Banner variant="alert" icon={<WarningIcon />} title="Drive scan problem" description={data.error} /> : null}

      <Section
        title="Storage Roles"
        description="Apps use a stable path per role. hearth recognises the same drive even if it's renamed or plugged into a different port, and pauses apps when it's unplugged."
      >
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
          {ROLES.map((r, i) => (
            <motion.div key={r.role} variants={riseIn} custom={i} initial="hidden" animate="show">
              <RoleCard meta={r} att={byRole.get(r.role)} drives={unattached} />
            </motion.div>
          ))}
        </div>
      </Section>

      <Section title="Connected Drives" description="Everything plugged in right now. Drives are detected automatically.">
        {data && data.drives.length === 0 ? (
          <Empty icon={<HardDrivesIcon size={40} />} title="No external drives" description="Plug in an SSD. It shows up here within a few seconds." />
        ) : (
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <AnimatePresence mode="popLayout">
              {data?.drives.map((d) => (
                <motion.div key={d.mount} layout initial={{ opacity: 0, scale: 0.97 }} animate={{ opacity: 1, scale: 1 }} exit={{ opacity: 0, scale: 0.97 }}>
                  <DriveCard d={d} />
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        )}
      </Section>
    </div>
  );
}

function RoleCard({ meta, att, drives }: { meta: (typeof ROLES)[number]; att?: AttachmentView; drives: Drive[] }) {
  const eject = useEject();
  const detach = useDetach();
  const [attachOpen, setAttachOpen] = useState(false);
  const [confirm, setConfirm] = useState<"detach" | null>(null);
  const I = meta.icon;
  const online = att?.status.state === "online";

  return (
    <Card className="flex h-full flex-col gap-4">
      <div className="flex items-start gap-3">
        <span className="inline-flex size-10 items-center justify-center rounded-lg bg-kumo-elevated ring-1 ring-kumo-hairline" aria-hidden="true">
          <I size={20} weight="duotone" />
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="font-semibold">{meta.title}</h3>
          <p className="text-xs text-kumo-subtle text-pretty">{meta.blurb}</p>
        </div>
      </div>

      {att ? (
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-2 text-sm">
            <StatusDot tone={online ? "success" : att.status.state === "offline" && att.status.impostor_at ? "danger" : "warning"} live={online} label={online ? "Online" : "Offline"} />
            <span className="font-medium">{att.label}</span>
            <span className="ml-auto text-xs text-kumo-subtle">{online ? "Connected" : `Last seen ${formatRelative(att.last_seen)}`}</span>
          </div>
          {online && att.total ? (
            <UsageBar label="Used" value={att.total - (att.free ?? 0)} max={att.total} detail={`${formatBytes(att.free)} free of ${formatBytes(att.total)}`} />
          ) : att.status.state === "offline" && att.status.impostor_at ? (
            <p className="rounded-lg bg-kumo-danger-tint p-2 text-xs text-kumo-danger">
              A different drive is mounted at {att.status.impostor_at}. hearth won’t use it.
            </p>
          ) : (
            <p className="text-xs text-kumo-subtle">Plug it back in and apps resume automatically.</p>
          )}
          <p className="truncate font-mono text-[11px] text-kumo-subtle" title={att.link} translate="no">
            {att.link}
          </p>
          {att.apps.length > 0 ? <p className="text-xs text-kumo-subtle">Used by {att.apps.join(", ")}</p> : null}
          <div className="mt-auto flex gap-2">
            {online ? (
              <Button size="sm" icon={<EjectIcon />} loading={eject.isPending} onClick={() => eject.mutate(att.id)}>
                Eject Safely
              </Button>
            ) : null}
            <Button size="sm" variant="secondary-destructive" icon={<LinkBreakIcon />} onClick={() => setConfirm("detach")}>
              Detach
            </Button>
          </div>
          {eject.error ?? detach.error ? <p className="text-xs text-kumo-danger">{(eject.error ?? detach.error)?.message}</p> : null}
        </div>
      ) : (
        <div className="mt-auto flex flex-col gap-2">
          <p className="text-sm text-kumo-subtle">No drive assigned.</p>
          <Button size="sm" variant="primary" icon={<PlugsConnectedIcon />} disabled={drives.length === 0} onClick={() => setAttachOpen(true)}>
            {drives.length === 0 ? "Plug in a Drive" : "Choose a Drive"}
          </Button>
        </div>
      )}

      {attachOpen ? <AttachDialog role={meta.role} title={meta.title} drives={drives} onClose={() => setAttachOpen(false)} /> : null}
      {confirm && att ? (
        <Dialog.Root open onOpenChange={(o) => !o && setConfirm(null)}>
          <Dialog size="base" className="flex flex-col gap-4 p-6">
            <Dialog.Title className="text-lg font-semibold">Detach {att.label}?</Dialog.Title>
            <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
              hearth stops using this drive as the {meta.title.toLowerCase()} drive. Nothing on it is deleted, and you can attach it again later.
            </Dialog.Description>
            <div className="flex justify-end gap-2">
              <Dialog.Close render={(p) => <Button {...p} variant="secondary">Keep It</Button>} />
              <Button variant="destructive" loading={detach.isPending} onClick={() => detach.mutate(att.id, { onSettled: () => setConfirm(null) })}>
                Detach Drive
              </Button>
            </div>
          </Dialog>
        </Dialog.Root>
      ) : null}
    </Card>
  );
}

function DriveCard({ d }: { d: Drive }) {
  return (
    <InteractiveCard className="flex flex-col gap-3">
      <div className="flex items-start gap-3">
        <HardDrivesIcon size={22} aria-hidden="true" className="mt-0.5 text-kumo-subtle" />
        <div className="min-w-0 flex-1">
          <h3 className="truncate font-semibold">{d.name}</h3>
          <p className="truncate font-mono text-xs text-kumo-subtle" translate="no">
            {d.mount}
          </p>
        </div>
        {d.attached_as ? (
          <Badge variant="success" appearance="dot">
            {d.attached_as}
          </Badge>
        ) : d.marker ? (
          <Badge variant="info">Known: was {d.marker.role}</Badge>
        ) : (
          <Badge variant="neutral">New</Badge>
        )}
      </div>
      <UsageBar label="Used" value={d.total - d.free} max={d.total} detail={`${formatBytes(d.free)} free of ${formatBytes(d.total)}`} />
      <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-kumo-subtle">
        <span className="uppercase">{d.filesystem}</span>
        {d.protocol ? <span>{d.protocol}</span> : null}
        <span>{d.writable ? "Writable" : "Read-only"}</span>
        {d.volume_uuid ? (
          <span className="font-mono" title={d.volume_uuid}>
            {d.volume_uuid.slice(0, 8)}…
          </span>
        ) : null}
      </div>
    </InteractiveCard>
  );
}

function AttachDialog({ role, title, drives, onClose }: { role: Role; title: string; drives: Drive[]; onClose: () => void }) {
  const attach = useAttach();
  const [mount, setMount] = useState(drives[0]?.mount ?? "");
  const [label, setLabel] = useState(drives[0]?.marker?.label ?? drives[0]?.name ?? "");
  const [force, setForce] = useState(false);
  const drive = drives.find((d) => d.mount === mount);
  const assessment = drive?.assessments.find(([r]) => r === role)?.[1];
  const denied = assessment?.level === "deny";

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="lg" className={DIALOG_FRAME}>
        <DialogFrame>
        <div className="flex flex-col gap-5">
        <div className="flex items-start justify-between gap-3">
          <div>
            <Dialog.Title className="text-lg font-semibold">Use a Drive as {title}</Dialog.Title>
            <Dialog.Description className="text-sm text-kumo-subtle">hearth writes a tiny marker file so it always recognises this drive.</Dialog.Description>
          </div>
          <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
        </div>

        <div role="radiogroup" aria-label="Drive" className="flex flex-col gap-2">
          <span className="text-sm font-medium">Drive</span>
          {drives.map((d) => {
            const on = mount === d.mount;
            return (
              <button
                key={d.mount}
                type="button"
                role="radio"
                aria-checked={on}
                onClick={() => {
                  setMount(d.mount);
                  setLabel(d.marker?.label ?? d.name);
                }}
                className={`flex items-center gap-3 rounded-xl p-3 text-left ring-1 transition-[box-shadow,background-color] ${on ? "bg-kumo-tint ring-kumo-brand" : "ring-kumo-hairline hover:bg-kumo-tint"}`}
              >
                <span aria-hidden="true" className={`flex size-4 items-center justify-center rounded-full ring-1 ${on ? "ring-kumo-brand" : "ring-kumo-line"}`}>
                  {on ? <span className="size-2 rounded-full bg-kumo-brand" /> : null}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block font-medium">{d.name}</span>
                  <span className="block text-xs text-kumo-subtle">
                    {formatBytes(d.total)} · {d.filesystem.toUpperCase()} {d.marker ? `· was “${d.marker.label}” (${d.marker.role})` : ""}
                  </span>
                </span>
              </button>
            );
          })}
        </div>

        <Field label="Label" hint="How it’s shown in hearth.">
          {(id) => <TextInput id={id} value={label} autoComplete="off" onChange={(e) => setLabel(e.target.value)} />}
        </Field>

        {assessment ? (
          <div
            className={`flex gap-2 rounded-xl p-3 text-sm ${assessment.level === "ok" ? "bg-kumo-success-tint" : assessment.level === "warn" ? "bg-kumo-warning-tint" : "bg-kumo-danger-tint"}`}
            aria-live="polite"
          >
            {assessment.level === "ok" ? <ShieldCheckIcon size={18} aria-hidden="true" className="text-kumo-success" /> : <WarningIcon size={18} aria-hidden="true" />}
            <ul className="flex flex-col gap-1">
              {assessment.notes.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </div>
        ) : null}
        {denied ? (
          <Checkbox label="Use it anyway. I understand the risks above." checked={force} onCheckedChange={(v) => setForce(v === true)} />
        ) : null}
        {attach.error ? <Banner variant="error" title="Couldn’t attach" description={attach.error.message} /> : null}

        <div className="flex justify-end gap-2">
          <Dialog.Close render={(p) => <Button {...p} variant="secondary">Cancel</Button>} />
          <Button
            variant="primary"
            icon={<PlugsConnectedIcon />}
            disabled={!drive || (denied && !force)}
            loading={attach.isPending}
            onClick={() => attach.mutate({ mount, role, label, force }, { onSuccess: onClose })}
          >
            {drive?.marker ? "Restore Drive" : `Use as ${title}`}
          </Button>
        </div>
        </div>
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}
