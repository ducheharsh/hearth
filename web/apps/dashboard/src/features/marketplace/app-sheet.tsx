import { useState } from "react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Switch } from "@cloudflare/kumo/components/switch";
import {
  ArrowClockwiseIcon,
  ArrowSquareOutIcon,
  GithubLogoIcon,
  GlobeIcon,
  PlayIcon,
  PlusIcon,
  StopIcon,
  XIcon,
} from "@phosphor-icons/react";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { Field, TextInput } from "@hearth/ui/components/form";
import { Stars } from "@hearth/ui/components/stars";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { formatCompact, formatRelative } from "@hearth/ui/format";
import { useAppAction, useAppHistory, useAutoUpdate, useOverride, useRate, useSettings, useUnpin, useUpdateApp, useUpdates } from "../../api/hooks";
import { Sparkline } from "@hearth/ui/components/sparkline";
import { formatBytes } from "@hearth/ui/format";
import type { CatalogApp } from "../../api/types";
import { FitBadge } from "./fit-badge";

interface AppSheetProps {
  app: CatalogApp;
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onInstall: () => void;
  onRemove: () => void;
}

/** App Store-style detail page: logo, rating, stats, controls and address settings. */
export function AppSheet({ app, open, onOpenChange, onInstall, onRemove }: AppSheetProps) {
  const rate = useRate();
  const action = useAppAction();
  const settings = useSettings();
  const domain = settings.data?.domain ?? null;

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog size="xl" className={DIALOG_FRAME}>
        <DialogFrame header={
        <div className="flex items-start gap-4">
          <AppLogo src={app.icon} name={app.name} category={app.category} size="lg" />
          <div className="min-w-0 flex-1">
            <Dialog.Title className="text-xl font-semibold tracking-tight">{app.name}</Dialog.Title>
            <p className="text-sm capitalize text-kumo-subtle">
              {app.category === "ai" ? "AI" : app.category}
              {app.custom ? " · your app" : ""}
            </p>
            <div className="mt-2 flex flex-wrap items-center gap-2">
              {app.installed ? (
                <Badge variant={app.running ? "success" : "neutral"} appearance="dot">
                  {app.running ? "Running" : "Stopped"}
                </Badge>
              ) : app.verdict ? (
                <FitBadge fit={app.verdict.fit} />
              ) : null}
              {app.featured ? <Badge variant="purple">Editor’s Choice</Badge> : null}
            </div>
          </div>
          <Dialog.Close render={(p) => <Button {...p} variant="ghost" shape="square" icon={<XIcon />} aria-label="Close" />} />
        </div>
        }>
        <div className="flex flex-col gap-6">

        <div className="flex flex-wrap gap-2">
          {app.installed ? (
            <>
              <LinkButton href={app.url} external variant="primary" icon={<ArrowSquareOutIcon />}>
                Open
              </LinkButton>
              {app.running ? (
                <>
                  <Button icon={<ArrowClockwiseIcon />} loading={action.isPending} onClick={() => action.mutate({ id: app.id, action: "restart" })}>
                    Restart
                  </Button>
                  <Button icon={<StopIcon />} loading={action.isPending} onClick={() => action.mutate({ id: app.id, action: "stop" })}>
                    Stop
                  </Button>
                </>
              ) : (
                <Button icon={<PlayIcon />} loading={action.isPending} onClick={() => action.mutate({ id: app.id, action: "start" })}>
                  Start
                </Button>
              )}
              <Button variant="secondary-destructive" onClick={onRemove}>
                Remove
              </Button>
            </>
          ) : (
            <Button variant="primary" icon={<PlusIcon />} onClick={onInstall}>
              Install
            </Button>
          )}
          {app.website ? (
            <LinkButton href={app.website} external variant="ghost" icon={<GlobeIcon />}>
              Website
            </LinkButton>
          ) : null}
          {app.repo ? (
            <LinkButton href={`https://github.com/${app.repo}`} external variant="ghost" icon={<GithubLogoIcon />}>
              Source
            </LinkButton>
          ) : null}
        </div>

        {app.editors_note ? (
          <blockquote className="rounded-xl bg-kumo-elevated p-4 text-sm text-pretty ring-1 ring-kumo-hairline">
            <span className="mb-1 block text-xs font-semibold uppercase tracking-wide text-kumo-subtle">Editor’s note</span>
            {app.editors_note}
          </blockquote>
        ) : null}
        <p className="text-sm text-pretty">{app.description}</p>

        <dl className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <Stat label="GitHub stars" value={app.stats?.stars != null ? `★ ${formatCompact(app.stats.stars)}` : "–"} />
          <Stat label="Updated" value={formatRelative(app.stats?.updated_at)} />
          <Stat label="Latest release" value={app.stats?.release ?? "–"} mono />
          <Stat label="RAM" value={`~${app.ram_mb} MB`} />
        </dl>

        <section className="flex flex-col gap-2" aria-label="Your rating">
          <h3 className="text-sm font-medium">Your Rating</h3>
          <Stars value={app.rating} onChange={(stars) => rate.mutate({ key: app.id, stars })} size={22} label={`Rate ${app.name}`} />
        </section>

        {app.installed ? <Updates id={app.id} /> : null}
        {app.installed ? <Usage id={app.id} /> : null}
        {app.installed ? <AddressSettings key={app.port} app={app} domain={domain} /> : null}
        </div>
        </DialogFrame>
      </Dialog>
    </Dialog.Root>
  );
}

function Stat({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-xl bg-kumo-elevated p-3 ring-1 ring-kumo-hairline">
      <dt className="text-xs text-kumo-subtle">{label}</dt>
      <dd className={mono ? "truncate font-mono text-sm" : "truncate text-sm font-semibold tabular-nums"}>{value}</dd>
    </div>
  );
}

/** Change the port, subdomain and public access for an installed app. */
function AddressSettings({ app, domain }: { app: CatalogApp; domain: string | null }) {
  const save = useOverride();
  const [port, setPort] = useState(String(app.port));
  const [sub, setSub] = useState(app.subdomain);
  const [pub, setPub] = useState(app.public);
  const portNum = Number(port);
  const dirty = portNum !== app.port || sub !== app.subdomain || pub !== app.public;
  return (
    <section className="flex flex-col gap-4 rounded-xl p-4 ring-1 ring-kumo-hairline" aria-label="Address">
      <div className="flex items-center gap-2">
        <StatusDot tone="success" label="Address" />
        <h3 className="text-sm font-medium">Address</h3>
      </div>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        <Field label="Tailnet port" hint={`https://…ts.net:${port || "?"}`}>
          {(id) => <TextInput id={id} inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))} autoComplete="off" />}
        </Field>
        <Field label="Subdomain" hint={domain ? `https://${sub || "?"}.${domain}` : "Connect a domain in Settings to use subdomains."}>
          {(id) => (
            <TextInput id={id} value={sub} spellCheck={false} autoComplete="off" disabled={!domain} onChange={(e) => setSub(e.target.value.toLowerCase())} />
          )}
        </Field>
      </div>
      <Switch
        label="Public on my domain (still behind my login)"
        checked={pub}
        disabled={!domain}
        onCheckedChange={setPub}
      />
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs text-kumo-subtle" aria-live="polite">
          {save.error ? <span className="text-kumo-danger">{save.error.message}</span> : app.domain_url ? `Live at ${app.domain_url}` : null}
        </span>
        <Button
          variant="primary"
          size="sm"
          disabled={!dirty || !portNum}
          loading={save.isPending}
          onClick={() => save.mutate({ id: app.id, o: { port: portNum, subdomain: sub, public: pub } })}
        >
          Save Address
        </Button>
      </div>
    </section>
  );
}

/** Memory/CPU over the last 7 days (5-minute buckets) and what the capacity check uses. */
function Usage({ id }: { id: string }) {
  const { data } = useAppHistory(id, true);
  const s = data?.summary;
  return (
    <section className="flex flex-col gap-3 rounded-xl p-4 ring-1 ring-kumo-hairline" aria-label="Resource usage">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h3 className="text-sm font-medium">Resource Usage</h3>
        <span className="text-xs text-kumo-subtle">{s ? `last ${s.hours >= 24 ? `${Math.round(s.hours / 24)} days` : `${Math.max(1, Math.round(s.hours))} h`}` : "collecting…"}</span>
      </div>
      {s && s.points.length > 1 ? (
        <>
          <Sparkline values={s.points.map((p) => p[1])} label="Memory over the last week" className="h-16" />
          <dl className="grid grid-cols-2 gap-2 text-sm sm:grid-cols-4">
            {[
              ["Average", formatBytes(s.avg_mem)],
              ["Typical peak (p95)", formatBytes(s.p95_mem)],
              ["Highest", formatBytes(s.peak_mem)],
              ["Avg CPU", `${s.avg_cpu.toFixed(1)}%`],
            ].map(([k, v]) => (
              <div key={k}>
                <dt className="text-xs text-kumo-subtle">{k}</dt>
                <dd className="font-semibold tabular-nums">{v}</dd>
              </div>
            ))}
          </dl>
        </>
      ) : (
        <p className="text-xs text-kumo-subtle">Usage history appears after the app has run for a few minutes.</p>
      )}
      <p className="text-xs text-kumo-subtle text-pretty">
        {data?.measured_mb != null
          ? `Capacity checks use the measured ${data.measured_mb} MB instead of the ${data.estimate_mb} MB estimate.`
          : `Capacity checks use the ${data?.estimate_mb ?? "–"} MB estimate until there’s an hour of data.`}
      </p>
    </section>
  );
}

/** Update status, one-click update (with automatic rollback), auto-update, unpin. */
function Updates({ id }: { id: string }) {
  const { data } = useUpdates();
  const update = useUpdateApp();
  const auto = useAutoUpdate();
  const unpin = useUnpin();
  const info = data?.apps[id];
  const running = data?.running.includes(id) ?? false;
  const last = data?.history.find((h) => h.app === id);
  const isAuto = data?.auto.includes(id) ?? false;
  return (
    <section className="flex flex-col gap-3 rounded-xl p-4 ring-1 ring-kumo-hairline" aria-label="Updates">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-sm font-medium">Updates</h3>
        {running ? (
          <Badge variant="info" appearance="dot">Updating…</Badge>
        ) : info?.pinned ? (
          <Badge variant="warning" appearance="dot">Held at previous version</Badge>
        ) : info?.available ? (
          <Badge variant="info" appearance="dot">Update available</Badge>
        ) : (
          <Badge variant="success" appearance="dot">Up to date</Badge>
        )}
      </div>
      <p className="text-xs text-kumo-subtle text-pretty">
        {info?.pinned
          ? "The last update wasn’t healthy, so hearth rolled back and is holding the previous images."
          : "Updating takes a backup first, then checks the app stays healthy. If it doesn’t, hearth rolls back automatically."}
      </p>
      {last ? <p className={`text-xs ${last.ok ? "text-kumo-subtle" : "text-kumo-warning"}`}>Last: {last.message}</p> : null}
      <div className="flex flex-wrap items-center gap-3">
        {info?.pinned ? (
          <Button size="sm" loading={unpin.isPending} onClick={() => unpin.mutate(id)}>
            Release Hold
          </Button>
        ) : (
          <Button size="sm" variant={info?.available ? "primary" : "secondary"} loading={running || update.isPending} onClick={() => update.mutate(id)}>
            {info?.available ? "Update Now" : "Reinstall Latest"}
          </Button>
        )}
        <Switch size="sm" label="Update automatically" checked={isAuto} onCheckedChange={(v) => auto.mutate({ id, enabled: v })} />
      </div>
    </section>
  );
}
