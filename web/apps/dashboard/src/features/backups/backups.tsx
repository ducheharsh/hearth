import { useEffect, useState } from "react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { ClipboardText } from "@cloudflare/kumo/components/clipboard-text";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Empty } from "@cloudflare/kumo/components/empty";
import { Loader } from "@cloudflare/kumo/components/loader";
import { SensitiveInput } from "@cloudflare/kumo/components/sensitive-input";
import { Switch } from "@cloudflare/kumo/components/switch";
import {
  ArrowCounterClockwiseIcon,
  CloudIcon,
  ClockCounterClockwiseIcon,
  HardDrivesIcon,
  LockKeyIcon,
  PlayIcon,
  ShieldCheckIcon,
  WarningIcon,
  XIcon,
} from "@phosphor-icons/react";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { Select, TextInput } from "@hearth/ui/components/form";
import { SettingRow, SettingsGroup, SettingsHeader } from "@hearth/ui/components/settings";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { formatBytes, formatRelative } from "@hearth/ui/format";
import {
  useBackupInit,
  useBackupRun,
  useBackups,
  useBackupSettings,
  useRestore,
  useSnapshots,
  type BackupSettings,
} from "../../api/hooks";

const HOURS = Object.fromEntries(Array.from({ length: 24 }, (_, h) => [String(h), `${String(h).padStart(2, "0")}:00`]));

export function Backups() {
  const { data, error } = useBackups();
  if (error) return <Banner variant="error" icon={<WarningIcon />} title="Can’t load backups" description={error.message} />;
  if (!data) return <Loader size={20} />;
  return (
    <div className="flex flex-col gap-8">
      <SettingsHeader
        title="Backups"
        description="Encrypted, deduplicated snapshots of every app’s data (restic). Restore any app to a previous night in one click."
      />
      {data.ready ? <Status /> : null}
      <Setup key={JSON.stringify(data.settings)} initial={data.settings} ready={data.ready} hasCreds={data.has_credentials} />
      {data.ready ? <AppList /> : null}
    </div>
  );
}

function Status() {
  const { data } = useBackups();
  const run = useBackupRun();
  const s = data?.settings;
  const lastAny = Math.max(0, ...(data?.apps.map((a) => a.last?.at ?? 0) ?? [0]));
  const failing = data?.apps.filter((a) => a.last && !a.last.ok).length ?? 0;
  return (
    <SettingsGroup
      title={failing ? `${failing} app${failing > 1 ? "s" : ""} failed to back up` : "Backups are healthy"}
      description={
        s?.enabled
          ? `Every night at ${HOURS[String(s.hour)]} · keeps ${s.retention.daily} daily, ${s.retention.weekly} weekly, ${s.retention.monthly} monthly`
          : "Nightly backups are off. Turn them on below, or back up manually."
      }
      aside={failing ? <Badge variant="error" appearance="dot">Attention</Badge> : <Badge variant="success" appearance="dot">{lastAny ? `Last ${formatRelative(lastAny)}` : "Ready"}</Badge>}
      footerActions={
        <Button size="xs" variant="primary" icon={<PlayIcon />} loading={run.isPending} onClick={() => run.mutate(null)}>
          Back Up Everything Now
        </Button>
      }
      footer="Each app is paused briefly while it’s backed up so databases are consistent."
    />
  );
}

const DESTS = [
  { kind: "local", title: "Backups drive", icon: HardDrivesIcon, blurb: "The SSD/HDD attached as Backups in Storage." },
  { kind: "s3", title: "S3-compatible", icon: CloudIcon, blurb: "AWS S3, Cloudflare R2, Wasabi, MinIO…" },
  { kind: "b2", title: "Backblaze B2", icon: CloudIcon, blurb: "Cheap offsite storage (~$6/TB/month)." },
] as const;

function Setup({ initial, ready, hasCreds }: { initial: BackupSettings; ready: boolean; hasCreds: boolean }) {
  const [s, setS] = useState(initial);
  const [keys, setKeys] = useState({ access_key_id: "", secret_access_key: "" });
  const save = useBackupSettings();
  const init = useBackupInit();
  const [password, setPassword] = useState<string | null>(null);
  const dirty = JSON.stringify(s) !== JSON.stringify(initial) || keys.access_key_id !== "" || keys.secret_access_key !== "";
  const d = s.destination;
  const setDest = (kind: string) =>
    setS({ ...s, destination: kind === "s3" ? { kind: "s3", endpoint: "", bucket: "", path: "hearth" } : kind === "b2" ? { kind: "b2", bucket: "", path: "hearth" } : { kind: "local" } });

  const submit = async () => {
    await save.mutateAsync({ settings: s, ...keys });
    setKeys({ access_key_id: "", secret_access_key: "" });
    if (!ready) {
      const r = await init.mutateAsync();
      setPassword(r.password);
    }
  };

  return (
    <>
      <SettingsGroup title="Where backups go" description="Ideally keep one copy off this machine (a different drive, or the cloud).">
        <div role="radiogroup" aria-label="Backup destination" className="grid grid-cols-1 gap-2 py-4 sm:grid-cols-3">
          {DESTS.map((o) => {
            const on = d.kind === o.kind;
            const I = o.icon;
            return (
              <button
                key={o.kind}
                type="button"
                role="radio"
                aria-checked={on}
                onClick={() => setDest(o.kind)}
                className={`flex items-start gap-3 rounded-xl p-3 text-left ring-1 transition-[box-shadow,background-color] ${on ? "bg-kumo-tint ring-kumo-brand" : "ring-kumo-hairline hover:bg-kumo-tint"}`}
              >
                <I size={20} weight="duotone" aria-hidden="true" className="mt-0.5 shrink-0" />
                <span className="min-w-0">
                  <span className="block text-sm font-medium">{o.title}</span>
                  <span className="block text-xs text-kumo-subtle text-pretty">{o.blurb}</span>
                </span>
              </button>
            );
          })}
        </div>
        {d.kind === "s3" ? (
          <SettingRow label="S3 bucket" description={hasCreds ? "Keys saved. Enter new ones to replace them." : "Use a key limited to this bucket."} stack>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
              <TextInput aria-label="Endpoint" placeholder="s3.us-east-1.amazonaws.com…" value={d.endpoint} spellCheck={false} autoComplete="off" onChange={(e) => setS({ ...s, destination: { ...d, endpoint: e.target.value } })} />
              <TextInput aria-label="Bucket" placeholder="bucket…" value={d.bucket} spellCheck={false} autoComplete="off" onChange={(e) => setS({ ...s, destination: { ...d, bucket: e.target.value } })} />
              <TextInput aria-label="Path in bucket" placeholder="hearth" value={d.path} spellCheck={false} autoComplete="off" onChange={(e) => setS({ ...s, destination: { ...d, path: e.target.value } })} />
              <TextInput aria-label="Access key ID" placeholder="Access key ID…" value={keys.access_key_id} autoComplete="off" onChange={(e) => setKeys({ ...keys, access_key_id: e.target.value })} />
              <SensitiveInput aria-label="Secret access key" placeholder="Secret access key…" value={keys.secret_access_key} onChange={(e: React.ChangeEvent<HTMLInputElement>) => setKeys({ ...keys, secret_access_key: e.target.value })} />
            </div>
          </SettingRow>
        ) : null}
        {d.kind === "b2" ? (
          <SettingRow label="B2 bucket" description={hasCreds ? "Keys saved. Enter new ones to replace them." : "Create an application key restricted to this bucket."} stack>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              <TextInput aria-label="Bucket" placeholder="bucket…" value={d.bucket} spellCheck={false} autoComplete="off" onChange={(e) => setS({ ...s, destination: { ...d, bucket: e.target.value } })} />
              <TextInput aria-label="Path in bucket" placeholder="hearth" value={d.path} spellCheck={false} autoComplete="off" onChange={(e) => setS({ ...s, destination: { ...d, path: e.target.value } })} />
              <TextInput aria-label="Key ID" placeholder="keyID…" value={keys.access_key_id} autoComplete="off" onChange={(e) => setKeys({ ...keys, access_key_id: e.target.value })} />
              <SensitiveInput aria-label="Application key" placeholder="applicationKey…" value={keys.secret_access_key} onChange={(e: React.ChangeEvent<HTMLInputElement>) => setKeys({ ...keys, secret_access_key: e.target.value })} />
            </div>
          </SettingRow>
        ) : null}
      </SettingsGroup>

      <SettingsGroup title="Schedule & retention">
        <SettingRow label="Nightly backups" description="Runs once a day at the chosen local time.">
          <span className="flex items-center gap-3">
            <Select aria-label="Backup time" size="sm" className="w-28" value={String(s.hour)} items={HOURS} onChange={(v) => setS({ ...s, hour: Number(v) })} />
            <Switch checked={s.enabled} onCheckedChange={(v) => setS({ ...s, enabled: v })} aria-label="Nightly backups" />
          </span>
        </SettingRow>
        <SettingRow label="Keep" description="Older snapshots are pruned automatically.">
          <span className="flex items-center gap-2 text-xs text-kumo-subtle">
            {(["daily", "weekly", "monthly"] as const).map((k) => (
              <label key={k} className="flex items-center gap-1">
                <TextInput aria-label={`${k} snapshots to keep`} inputMode="numeric" className="w-14 text-right tabular-nums" value={String(s.retention[k])} onChange={(e) => setS({ ...s, retention: { ...s.retention, [k]: Number(e.target.value.replace(/\D/g, "")) || 0 } })} />
                {k}
              </label>
            ))}
          </span>
        </SettingRow>
        <SettingRow label="Pause apps during their backup" description="Recommended: guarantees database files are consistent. Takes seconds per app.">
          <Switch checked={s.stop_apps} onCheckedChange={(v) => setS({ ...s, stop_apps: v })} aria-label="Pause apps during backup" />
        </SettingRow>
        <SettingRow label="Include media libraries" description="Movies and music are huge and usually re-downloadable.">
          <Switch checked={s.include_media} onCheckedChange={(v) => setS({ ...s, include_media: v })} aria-label="Include media" />
        </SettingRow>
      </SettingsGroup>

      {save.error ?? init.error ? <Banner variant="error" title="Couldn’t save" description={(save.error ?? init.error)?.message} /> : null}
      {dirty || !ready ? (
        <div className="flex justify-end">
          <Button variant="primary" icon={ready ? undefined : <ShieldCheckIcon />} loading={save.isPending || init.isPending} onClick={() => void submit()}>
            {ready ? "Save Backup Settings" : "Set Up Backups"}
          </Button>
        </div>
      ) : null}

      {password ? (
        <Dialog.Root open onOpenChange={(o) => !o && setPassword(null)}>
          <Dialog size="lg" className={DIALOG_FRAME}>
            <DialogFrame
              header={
                <div className="flex items-start gap-3">
                  <LockKeyIcon size={24} weight="duotone" aria-hidden="true" className="text-kumo-warning" />
                  <div>
                    <Dialog.Title className="text-lg font-semibold">Save your recovery password</Dialog.Title>
                    <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
                      Backups are encrypted with this password. If this Mac is lost, you need it to restore anything. Put it in your password manager now.
                    </Dialog.Description>
                  </div>
                </div>
              }
              footer={
                <Button variant="primary" onClick={() => setPassword(null)}>
                  I’ve Saved It
                </Button>
              }
            >
              <ClipboardText text={password} />
            </DialogFrame>
          </Dialog>
        </Dialog.Root>
      ) : null}
    </>
  );
}

function AppList() {
  const { data } = useBackups();
  const run = useBackupRun();
  const [open, setOpen] = useState<{ id: string; name: string } | null>(null);
  const apps = data?.apps ?? [];
  return (
    <SettingsGroup title="Apps" description="Every installed app is included automatically.">
      {apps.length === 0 ? (
        <div className="py-4">
          <Empty size="sm" icon={<ClockCounterClockwiseIcon size={32} />} title="No apps yet" description="Install an app and it’s backed up from the next run." />
        </div>
      ) : (
        apps.map((a) => (
          <SettingRow
            key={a.id}
            label={a.name}
            description={
              a.running ? (
                <span className="inline-flex items-center gap-1.5">
                  <Loader size={12} /> Backing up…
                </span>
              ) : a.last ? (
                <span className="inline-flex items-center gap-1.5">
                  <StatusDot tone={a.last.ok ? "success" : "danger"} label={a.last.ok ? "OK" : "Failed"} />
                  {a.last.ok
                    ? `${formatRelative(a.last.at)} · +${formatBytes(a.last.bytes_added ?? 0)} · ${a.last.seconds}s`
                    : `Failed ${formatRelative(a.last.at)}: ${a.last.error ?? ""}`}
                </span>
              ) : (
                "Not backed up yet"
              )
            }
          >
            <span className="flex gap-2">
              <AppLogo src={`/api/icons/${a.icon_file}`} name={a.name} category="" size="sm" className="hidden sm:inline-flex" />
              <Button size="sm" icon={<PlayIcon />} disabled={a.running} onClick={() => run.mutate(a.id)}>
                Back Up
              </Button>
              <Button size="sm" variant="secondary" icon={<ArrowCounterClockwiseIcon />} onClick={() => setOpen({ id: a.id, name: a.name })}>
                Restore…
              </Button>
            </span>
          </SettingRow>
        ))
      )}
      {open ? <SnapshotsDialog app={open} onClose={() => setOpen(null)} /> : null}
    </SettingsGroup>
  );
}

function SnapshotsDialog({ app, onClose }: { app: { id: string; name: string }; onClose: () => void }) {
  const { data, error, isPending } = useSnapshots(app.id);
  const restore = useRestore();
  const [confirm, setConfirm] = useState<string | null>(null);
  useEffect(() => {
    if (restore.isSuccess) onClose();
  }, [restore.isSuccess, onClose]);
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="lg" className={DIALOG_FRAME}>
        <DialogFrame
          header={
            <div className="flex items-start justify-between gap-3">
              <div>
                <Dialog.Title className="text-lg font-semibold">Restore {app.name}</Dialog.Title>
                <Dialog.Description className="text-sm text-kumo-subtle">Pick a snapshot. The app is stopped, its files are put back exactly, then it starts again.</Dialog.Description>
              </div>
              <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
            </div>
          }
        >
          {isPending ? <Loader size={18} /> : null}
          {error ? <Banner variant="error" title="Can’t list snapshots" description={error.message} /> : null}
          {data?.length === 0 ? <p className="text-sm text-kumo-subtle">No snapshots yet.</p> : null}
          <ul className="flex flex-col divide-y divide-kumo-hairline">
            {data?.map((s) => (
              <li key={s.id} className="flex flex-wrap items-center gap-3 py-3">
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium">{new Date(s.time).toLocaleString()}</span>
                  <span className="block text-xs text-kumo-subtle">
                    {formatRelative(s.time)} · {s.tags.filter((t) => !t.startsWith("app:") && t !== "hearth").join(", ") || "snapshot"} · {s.bytes != null ? formatBytes(s.bytes) : ""} · <span className="font-mono">{s.short_id}</span>
                  </span>
                </span>
                {confirm === s.id ? (
                  <span className="flex gap-2">
                    <Button size="sm" variant="secondary" onClick={() => setConfirm(null)}>
                      Cancel
                    </Button>
                    <Button size="sm" variant="destructive" loading={restore.isPending} onClick={() => restore.mutate({ id: app.id, snapshot: s.short_id })}>
                      Replace Current Data
                    </Button>
                  </span>
                ) : (
                  <Button size="sm" onClick={() => setConfirm(s.id)}>
                    Restore
                  </Button>
                )}
              </li>
            ))}
          </ul>
          {restore.error ? <Banner variant="error" title="Restore failed" description={restore.error.message} /> : null}
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}
