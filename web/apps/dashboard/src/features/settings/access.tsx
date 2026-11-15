import { useState, type FormEvent } from "react";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Switch } from "@cloudflare/kumo/components/switch";
import { Tabs } from "@cloudflare/kumo/components/tabs";
import { ArrowsClockwiseIcon, TrashIcon, UserPlusIcon } from "@phosphor-icons/react";
import { TextInput } from "@hearth/ui/components/form";
import { SettingRow, SettingsGroup, SettingsHeader } from "@hearth/ui/components/settings";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { useAddUser, useDeleteUser, useGateway, useGatewayAction, useTailscale, useUsers } from "../../api/hooks";

export function AccessSection() {
  const gw = useGateway();
  const users = useUsers();
  const ts = useTailscale();
  const action = useGatewayAction();
  const enabled = gw.data?.enabled ?? false;
  const host = ts.data?.dns_name ?? "your-machine.ts.net";
  const noUsers = (users.data?.length ?? 0) === 0;

  return (
    <>
      <SettingsHeader title="Logins & Gateway" description="One login, with passkeys or an authenticator app, in front of every app. The gateway is what publishes apps to your devices." />

      <SettingsGroup
        title="Gateway"
        description="Runs Caddy and Authelia, then publishes every app with HTTPS on your private network."
        aside={
          <Switch
            checked={enabled}
            disabled={action.isPending || (!enabled && noUsers)}
            onCheckedChange={(on) => action.mutate(on ? "enable" : "disable")}
            aria-label="Gateway"
          />
        }
        footer={noUsers && !enabled ? "Create a login below first." : enabled ? "Changes to apps re-apply automatically." : "Off: nothing is published."}
        footerActions={
          enabled ? (
            <Button size="xs" variant="secondary" icon={<ArrowsClockwiseIcon />} loading={action.isPending} onClick={() => action.mutate("apply")}>
              Re-apply
            </Button>
          ) : null
        }
      >
        {enabled ? (
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
  const [u, setU] = useState({ username: "", display_name: "", email: "", password: "" });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    add.mutate(u, { onSuccess: () => setU({ username: "", display_name: "", email: "", password: "" }) });
  };
  return (
    <SettingsGroup title="Logins" description="Everyone who can use your apps. Passwords are hashed with argon2id; 2FA is set up on first sign-in.">
      {users.data?.map((x) => (
        <SettingRow key={x.username} label={x.display_name} description={`${x.username} · ${x.email}`}>
          <Button size="xs" variant="ghost" shape="square" icon={<TrashIcon />} aria-label={`Delete ${x.username}`} loading={remove.isPending} onClick={() => remove.mutate(x.username)} />
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
