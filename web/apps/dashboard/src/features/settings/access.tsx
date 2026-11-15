import { AnimatePresence } from "motion/react";
import { useState, type FormEvent } from "react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Switch } from "@cloudflare/kumo/components/switch";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import { ArrowsClockwiseIcon, DeviceMobileIcon, TrashIcon, UserPlusIcon } from "@phosphor-icons/react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { TextInput } from "@hearth/ui/components/form";
import { SettingRow, SettingsGroup, SettingsHeader } from "@hearth/ui/components/settings";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { useAddUser, useDeleteUser, useGateway, useGatewayAction, useTailscale, useTotpRemove, useUsers } from "../../api/hooks";
import { TwoFactorDialog } from "./two-factor";
import { GatewayProgress } from "./gateway-progress";

export function AccessSection() {
  const gw = useGateway();
  const users = useUsers();
  const ts = useTailscale();
  const action = useGatewayAction();
  const job = gw.data?.job ?? null;
  const setting = job != null && job.finished == null;
  const enabled = (gw.data?.enabled ?? false) || setting;
  // Show the progress while it runs, after a failure, and briefly after success.
  const [dismissed, setDismissed] = useState<number | null>(null);
  const recent = job?.finished != null && Date.now() / 1000 - job.finished < 120;
  const showJob = job != null && dismissed !== job.started && (setting || !job.ok || recent);
  const host = ts.data?.dns_name ?? "your-machine.ts.net";
  const noUsers = (users.data?.length ?? 0) === 0;
  const tsUp = ts.data?.backend_state === "Running";
  const httpsOff = tsUp && !ts.data?.https_enabled;
  const blocked = noUsers
    ? "Create a login below first."
    : !tsUp
      ? "Connect Tailscale first (Settings → Network)."
      : httpsOff
        ? "Turn on MagicDNS and HTTPS Certificates for your tailnet first (login.tailscale.com/admin/dns)."
        : null;

  return (
    <>
      <SettingsHeader title="Logins & Gateway" description="One login, with passkeys or an authenticator app, in front of every app. The gateway is what publishes apps to your devices." />

      <SettingsGroup
        title="Gateway"
        description="Needed to open hearth and your apps from other devices. Runs Caddy and Authelia (your login), then publishes everything with HTTPS on your Tailscale network. Requires Tailscale with HTTPS certificates and at least one login."
        aside={
          <Switch
            checked={enabled}
            disabled={setting || action.isPending || (!enabled && blocked != null)}
            onCheckedChange={(on) => action.mutate(on ? "enable" : "disable")}
            aria-label="Gateway"
          />
        }
        footer={
          setting
            ? "Setting up… you can leave this page."
            : !enabled && blocked
              ? blocked
              : enabled
                ? "Changes to apps re-apply automatically."
                : "Off: nothing is published."
        }
        footerActions={
          enabled && !setting ? (
            <Button size="xs" variant="secondary" icon={<ArrowsClockwiseIcon />} loading={action.isPending} onClick={() => action.mutate("apply")}>
              Re-apply
            </Button>
          ) : null
        }
      >
        <AnimatePresence initial={false}>
          {showJob && job ? (
            <GatewayProgress key={job.started} job={job} retrying={action.isPending} onRetry={() => action.mutate("enable")} onDismiss={() => setDismissed(job.started)} />
          ) : null}
        </AnimatePresence>
        {enabled && !setting ? (
          <>
            <SettingRow label="Sign-in page" description="Where you log in and register 2FA.">
              <a className="font-mono text-sm text-kumo-link underline" href={`https://${host}:9091`} target="_blank" rel="noopener noreferrer">
                https://{host}:9091
              </a>
            </SettingRow>
            <SettingRow label="This dashboard" description="Reach hearth itself from any of your devices.">
              <a className="font-mono text-sm text-kumo-link underline" href={`https://${host}:9443`} target="_blank" rel="noopener noreferrer">
                https://{host}:9443
              </a>
            </SettingRow>
          </>
        ) : null}
        {action.error ? (
          <div className="py-3">
            <Banner variant="error" title="Gateway problem" description={action.error.message} />
          </div>
        ) : null}
      </SettingsGroup>

      <Logins />

      {gw.data ? <Preview preview={gw.data.preview} /> : null}
    </>
  );
}

function Logins() {
  const users = useUsers();
  const add = useAddUser();
  const remove = useDeleteUser();
  const totpRemove = useTotpRemove();
  const [enroll, setEnroll] = useState<string | null>(null);
  const [u, setU] = useState({ username: "", display_name: "", email: "", password: "" });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    add.mutate(u, { onSuccess: () => setU({ username: "", display_name: "", email: "", password: "" }) });
  };
  return (
    <SettingsGroup
      title="Logins"
      description="Everyone who can use your apps. Passwords are hashed with argon2id. Two-factor uses an authenticator app; passkeys can also be added on the sign-in page."
      footer={totpRemove.error ? <span className="text-kumo-danger">{totpRemove.error.message}</span> : undefined}
    >
      {enroll ? <TwoFactorDialog user={enroll} onClose={() => setEnroll(null)} /> : null}
      {users.data?.map((x) => (
        <SettingRow key={x.username} label={x.display_name} description={`${x.username} · ${x.email}`}>
          <span className="flex items-center gap-2">
            {x.totp ? (
              <>
                <Badge variant="success" appearance="dot">2FA on</Badge>
                <Button size="xs" variant="ghost" onClick={() => setEnroll(x.username)}>
                  Reset
                </Button>
                <Button size="xs" variant="ghost" loading={totpRemove.isPending} onClick={() => totpRemove.mutate(x.username)}>
                  Remove
                </Button>
              </>
            ) : (
              <Button size="xs" variant="secondary" icon={<DeviceMobileIcon />} onClick={() => setEnroll(x.username)}>
                Set Up 2FA
              </Button>
            )}
            <Button size="xs" variant="ghost" shape="square" icon={<TrashIcon />} aria-label={`Delete ${x.username}`} loading={remove.isPending} onClick={() => remove.mutate(x.username)} />
          </span>
        </SettingRow>
      ))}
      <SettingRow label="Add or update a login" description="Typing an existing username changes its password." stack>
        <form onSubmit={submit} className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          <TextInput aria-label="Username" name="username" required autoComplete="username" spellCheck={false} placeholder="username…" value={u.username} onChange={(e) => setU({ ...u, username: e.target.value.toLowerCase() })} />
          <TextInput aria-label="Display name" name="name" autoComplete="name" placeholder="Display name…" value={u.display_name} onChange={(e) => setU({ ...u, display_name: e.target.value })} />
          <TextInput aria-label="Email" name="email" type="email" required autoComplete="email" spellCheck={false} placeholder="email (for 2FA setup)…" value={u.email} onChange={(e) => setU({ ...u, email: e.target.value })} />
          <TextInput aria-label="Password" name="new-password" type="password" required minLength={12} autoComplete="new-password" placeholder="Password (12+ characters)…" value={u.password} onChange={(e) => setU({ ...u, password: e.target.value })} />
          {add.error ? <p className="text-xs text-kumo-danger sm:col-span-2">{add.error.message}</p> : null}
          <Button type="submit" size="sm" variant="primary" icon={<UserPlusIcon />} loading={add.isPending} className="justify-self-start">
            Save Login
          </Button>
        </form>
      </SettingRow>
    </SettingsGroup>
  );
}

function Preview({ preview }: { preview: { routes: { id: string; name: string; port: number; upstream: string; protect: boolean; public: boolean }[]; caddyfile: string; authelia: string } }) {
  const [tab, setTab] = useState("routes");
  return (
    <SettingsGroup title="What gets published" description="Read-only view of the generated gateway configuration.">
      <div className="flex flex-col gap-3 py-4">
        <Tabs
          variant="underline"
          size="sm"
          value={tab}
          onValueChange={setTab}
          tabs={[
            { value: "routes", label: "Routes" },
            { value: "caddy", label: "Caddyfile" },
            { value: "authelia", label: "Authelia" },
          ]}
        />
        {tab === "routes" ? (
          <ul className="divide-y divide-kumo-hairline text-sm">
            {preview.routes.map((r) => (
              <li key={r.id} className="flex items-center gap-3 py-2">
                <StatusDot tone={r.protect ? "success" : "warning"} label={r.protect ? "Login required" : "No login"} />
                <span className="flex-1">{r.name}</span>
                <span className="font-mono text-xs text-kumo-subtle">
                  :{r.port} → {r.upstream}
                </span>
              </li>
            ))}
          </ul>
        ) : (
          <pre className="max-h-96 overflow-auto rounded-lg bg-kumo-recessed p-3 font-mono text-xs">{tab === "caddy" ? preview.caddyfile : preview.authelia}</pre>
        )}
      </div>
    </SettingsGroup>
  );
}
