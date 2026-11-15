import { useState } from "react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { ClipboardText } from "@cloudflare/kumo/components/clipboard-text";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Switch } from "@cloudflare/kumo/components/switch";
import {
  ArrowSquareOutIcon,
  DownloadSimpleIcon,
  KeyIcon,
  PlugIcon,
  PlugsIcon,
  SignInIcon,
  SignOutIcon,
  UserPlusIcon,
} from "@phosphor-icons/react";
import { Select, TextInput } from "@hearth/ui/components/form";
import {
  SettingRow,
  SettingsGroup,
  SettingsHeader,
} from "@hearth/ui/components/settings";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { formatRelative } from "@hearth/ui/format";
import {
  useHeadscale,
  useHeadscaleInstall,
  useHeadscaleJoinSelf,
  useHeadscaleKey,
  useHeadscaleUninstall,
  useHeadscaleUser,
  useLoginServer,
  useNetbird,
  useNetbirdAction,
  useSettings,
  useTailscale,
  useTailscaleAction,
} from "../../api/hooks";

export function NetworkSection() {
  return (
    <>
      <SettingsHeader
        title="Network"
        description="How your devices reach this server privately. Tailscale is the easiest; NetBird and Headscale are fully open-source alternatives you can run yourself."
      />
      <TailscaleGroup />
      <NetbirdGroup />
      <HeadscaleGroup />
    </>
  );
}

function Confirm({
  title,
  body,
  action,
  onConfirm,
  onClose,
  pending,
}: {
  title: string;
  body: string;
  action: string;
  onConfirm: () => void;
  onClose: () => void;
  pending: boolean;
}) {
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="base" className="flex flex-col gap-4 p-6">
        <Dialog.Title className="text-lg font-semibold">{title}</Dialog.Title>
        <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
          {body}
        </Dialog.Description>
        <div className="flex justify-end gap-2">
          <Dialog.Close
            render={(p) => (
              <Button {...p} variant="secondary">
                Cancel
              </Button>
            )}
          />
          <Button variant="destructive" loading={pending} onClick={onConfirm}>
            {action}
          </Button>
        </div>
      </Dialog>
    </Dialog.Root>
  );
}

function TailscaleGroup() {
  const { data: ts } = useTailscale();
  const act = useTailscaleAction();
  const loginServer = useLoginServer();
  const [name, setName] = useState("");
  const [custom, setCustom] = useState({ url: "", key: "" });
  const [confirm, setConfirm] = useState<null | "logout" | "reset">(null);
  const running = ts?.backend_state === "Running";

  return (
    <SettingsGroup
      title="Tailscale"
      description="Encrypted access from your phone and laptop, with automatic HTTPS for every app."
      aside={
        ts ? (
          <Badge variant={running ? "success" : "warning"} appearance="dot">
            {!ts.installed
              ? "Not installed"
              : running
                ? "Connected"
                : ts.backend_state}
          </Badge>
        ) : null
      }
      footer={
        ts?.version
          ? `Tailscale ${ts.version.split("-")[0]}${ts.tailnet ? ` · tailnet ${ts.tailnet}` : ""}`
          : "Install from tailscale.com/download"
      }
      footerActions={
        running ? (
          <Button
            size="xs"
            variant="secondary-destructive"
            icon={<SignOutIcon />}
            onClick={() => setConfirm("logout")}
          >
            Log Out
          </Button>
        ) : null
      }
    >
      {ts?.auth_url ? (
        <div className="py-4">
          <Banner
            title="Finish logging in"
            description="Approve this machine in your browser; this page updates by itself."
            action={
              <LinkButton
                href={ts.auth_url}
                external
                variant="primary"
                size="sm"
                icon={<ArrowSquareOutIcon />}
              >
                Open Login Link
              </LinkButton>
            }
          />
        </div>
      ) : null}
      <SettingRow
        label="Connection"
        description={
          running
            ? `${ts?.dns_name ?? ""} · ${ts?.ips[0] ?? ""}`
            : "Not connected to a tailnet."
        }
      >
        {!ts?.installed ? null : ts.backend_state === "NeedsLogin" ||
          ts.backend_state === "NoState" ? (
          <Button
            variant="primary"
            size="sm"
            icon={<SignInIcon />}
            loading={act.isPending}
            onClick={() => act.mutate({ path: "login" })}
          >
            Log In
          </Button>
        ) : running ? (
          <Button
            size="sm"
            icon={<PlugsIcon />}
            loading={act.isPending}
            onClick={() => act.mutate({ path: "down" })}
          >
            Disconnect
          </Button>
        ) : (
          <Button
            size="sm"
            variant="primary"
            icon={<PlugIcon />}
            loading={act.isPending}
            onClick={() => act.mutate({ path: "up" })}
          >
            Connect
          </Button>
        )}
      </SettingRow>
      {running ? (
        <>
          <SettingRow
            label="HTTPS for apps"
            description="Needs MagicDNS and HTTPS Certificates enabled for your tailnet."
          >
            <span className="flex items-center gap-3 text-sm">
              <span className="inline-flex items-center gap-1.5">
                <StatusDot
                  tone={ts.magic_dns ? "success" : "warning"}
                  label="MagicDNS"
                />{" "}
                MagicDNS
              </span>
              <span className="inline-flex items-center gap-1.5">
                <StatusDot
                  tone={ts.https_enabled ? "success" : "warning"}
                  label="HTTPS"
                />{" "}
                HTTPS
              </span>
              {!ts.magic_dns || !ts.https_enabled ? (
                <LinkButton
                  href="https://login.tailscale.com/admin/dns"
                  external
                  size="xs"
                  variant="secondary"
                  icon={<ArrowSquareOutIcon />}
                >
                  Enable
                </LinkButton>
              ) : null}
            </span>
          </SettingRow>
          <SettingRow
            label="Machine name"
            description="Changes every app’s address (name.tailnet.ts.net)."
            htmlFor="ts-name"
          >
            <form
              className="flex gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                act.mutate(
                  { path: "hostname", body: { hostname: name } },
                  { onSuccess: () => setName("") },
                );
              }}
            >
              <TextInput
                id="ts-name"
                className="w-44"
                value={name}
                placeholder={ts.hostname ?? "m1…"}
                spellCheck={false}
                autoComplete="off"
                onChange={(e) => setName(e.target.value.toLowerCase())}
              />
              <Button
                type="submit"
                size="sm"
                disabled={!name.trim()}
                loading={act.isPending}
              >
                Rename
              </Button>
            </form>
          </SettingRow>
          <SettingRow
            label="Published ports"
            description="What hearth exposes on your tailnet. Clear only if something looks wrong."
          >
            <Button
              size="sm"
              variant="secondary-destructive"
              onClick={() => setConfirm("reset")}
            >
              Clear All
            </Button>
          </SettingRow>
        </>
      ) : null}
      {ts?.installed ? (
        <SettingRow
          label="Use a custom control server"
          description="Join a Headscale server instead of Tailscale’s. Needs its URL and a pre-auth key."
          stack
        >
          <form
            className="grid grid-cols-1 gap-2 sm:grid-cols-[1fr_1fr_auto]"
            onSubmit={(e) => {
              e.preventDefault();
              loginServer.mutate({
                login_server: custom.url,
                auth_key: custom.key,
              });
            }}
          >
            <TextInput
              aria-label="Control server URL"
              placeholder="https://hs.example.com…"
              value={custom.url}
              spellCheck={false}
              autoComplete="off"
              onChange={(e) => setCustom({ ...custom, url: e.target.value })}
            />
            <TextInput
              aria-label="Pre-auth key"
              type="password"
              placeholder="Pre-auth key…"
              value={custom.key}
              autoComplete="off"
              onChange={(e) => setCustom({ ...custom, key: e.target.value })}
            />
            <Button
              type="submit"
              size="sm"
              disabled={!custom.url || !custom.key}
              loading={loginServer.isPending}
            >
              Join
            </Button>
          </form>
          {loginServer.error ? (
            <p className="text-xs text-kumo-danger">
              {loginServer.error.message}
            </p>
          ) : null}
        </SettingRow>
      ) : null}
      {act.error ? (
        <div className="py-3">
          <Banner
            variant="error"
            title="Tailscale command failed"
            description={act.error.message}
          />
        </div>
      ) : null}
      {confirm ? (
        <Confirm
          title={
            confirm === "logout"
              ? "Log out of Tailscale?"
              : "Clear published ports?"
          }
          body={
            confirm === "logout"
              ? "Every device loses access until you log in again. If you’re connected over Tailscale, this page disconnects."
              : "Apps stop being reachable until the gateway is re-applied."
          }
          action={confirm === "logout" ? "Log Out" : "Clear Ports"}
          pending={act.isPending}
          onClose={() => setConfirm(null)}
          onConfirm={() =>
            act.mutate(
              { path: confirm === "logout" ? "logout" : "serve/reset" },
              { onSettled: () => setConfirm(null) },
            )
          }
        />
      ) : null}
    </SettingsGroup>
  );
}

function NetbirdGroup() {
  const { data: nb } = useNetbird();
  const settings = useSettings();
  const act = useNetbirdAction();
  const [key, setKey] = useState("");
  const [mgmt, setMgmt] = useState("");
  const exposed = settings.data?.netbird_enabled ?? false;

  return (
    <SettingsGroup
      title="NetBird"
      description="Open-source WireGuard mesh, self-hostable. Apps are served with HTTPS from hearth’s own certificate authority."
      aside={
        nb ? (
          <Badge
            variant={
              nb.connected ? "success" : nb.installed ? "warning" : "neutral"
            }
            appearance="dot"
          >
            {!nb.installed
              ? "Not installed"
              : nb.connected
                ? "Connected"
                : "Disconnected"}
          </Badge>
        ) : null
      }
      footer={
        nb?.installed ? (
          `NetBird ${nb.version ?? ""}${nb.management_url ? ` · ${nb.management_url}` : ""}`
        ) : (
          <>
            Install with <code>brew install netbirdio/tap/netbird</code> then{" "}
            <code>
              sudo netbird service install && sudo netbird service start
            </code>
            .
          </>
        )
      }
      footerActions={
        <LinkButton
          href="https://docs.netbird.io/how-to/getting-started"
          external
          size="xs"
          variant="secondary"
          icon={<ArrowSquareOutIcon />}
        >
          Guide
        </LinkButton>
      }
    >
      {nb?.installed && !nb.connected ? (
        <SettingRow
          label="Connect"
          description="Create a setup key in the NetBird dashboard (Setup Keys). Leave the URL empty for NetBird Cloud."
          stack
        >
          <form
            className="grid grid-cols-1 gap-2 sm:grid-cols-[1fr_1fr_auto]"
            onSubmit={(e) => {
              e.preventDefault();
              act.mutate(
                {
                  path: "up",
                  body: { setup_key: key, management_url: mgmt || null },
                },
                { onSuccess: () => setKey("") },
              );
            }}
          >
            <TextInput
              aria-label="Setup key"
              type="password"
              placeholder="Setup key…"
              value={key}
              autoComplete="off"
              onChange={(e) => setKey(e.target.value)}
            />
            <TextInput
              aria-label="Management URL"
              placeholder="https://netbird.example.com (optional)…"
              value={mgmt}
              spellCheck={false}
              autoComplete="off"
              onChange={(e) => setMgmt(e.target.value)}
            />
            <Button
              type="submit"
              size="sm"
              variant="primary"
              disabled={!key}
              loading={act.isPending}
            >
              Connect
            </Button>
          </form>
        </SettingRow>
      ) : null}
      {nb?.connected ? (
        <>
          <SettingRow
            label="Connection"
            description={`${nb.fqdn ?? ""} · ${nb.ip ?? ""} · ${nb.peers_connected}/${nb.peers_total} peers online`}
          >
            <Button
              size="sm"
              icon={<PlugsIcon />}
              loading={act.isPending}
              onClick={() => act.mutate({ path: "down" })}
            >
              Disconnect
            </Button>
          </SettingRow>
          <SettingRow
            label="Serve apps on NetBird"
            description={`Apps at https://${nb.fqdn ?? "…"}:<port>, behind your login. Requires the gateway to be on.`}
          >
            <Switch
              checked={exposed}
              disabled={act.isPending}
              onCheckedChange={(v) =>
                act.mutate({ path: "expose", body: { enabled: v } })
              }
              aria-label="Serve apps on NetBird"
            />
          </SettingRow>
          {exposed ? (
            <SettingRow
              label="Trust hearth’s certificate"
              description="Install this once on each device (and mark it trusted) so browsers accept NetBird HTTPS."
            >
              <LinkButton
                href="/api/gateway/ca.crt"
                size="sm"
                icon={<DownloadSimpleIcon />}
              >
                Download CA
              </LinkButton>
            </SettingRow>
          ) : null}
        </>
      ) : null}
      {act.error ? (
        <div className="py-3">
          <Banner
            variant="error"
            title="NetBird command failed"
            description={act.error.message}
          />
        </div>
      ) : null}
    </SettingsGroup>
  );
}

const STEPS = [
  {
    title: "Public hostname",
    body: "Create a DNS A record like hs.example.com pointing to your home’s public IP (DNS only; Cloudflare proxy must be OFF).",
  },
  {
    title: "Forward ports",
    body: "On your router, forward TCP 80 and 443 to this Mac. Let’s Encrypt uses port 80 to issue the certificate.",
  },
  {
    title: "Install",
    body: "hearth writes the config and runs Headscale with automatic HTTPS.",
  },
  {
    title: "Add people & devices",
    body: "Create a user, generate a key, and run the join command on each device.",
  },
];

function HeadscaleGroup() {
  const { data: hs } = useHeadscale();
  const install = useHeadscaleInstall();
  const uninstall = useHeadscaleUninstall();
  const [form, setForm] = useState({
    hostname: "",
    acme_email: "",
    base_domain: "tail.internal",
  });
  const [stop, setStop] = useState(false);
  const installed = Boolean(hs?.config);

  return (
    <SettingsGroup
      title="Headscale"
      description="Run your own Tailscale control server, so your network doesn’t depend on Tailscale’s service. Use it with the regular Tailscale apps."
      aside={
        hs ? (
          <Badge
            variant={
              hs.running ? (hs.reachable ? "success" : "warning") : "neutral"
            }
            appearance="dot"
          >
            {!installed
              ? "Not set up"
              : !hs.running
                ? "Stopped"
                : hs.reachable
                  ? "Running"
                  : "Not reachable"}
          </Badge>
        ) : null
      }
      footer={
        <>
          Headscale ({hs?.image.split(":")[1] ?? ""}) · Cloudflare Tunnel and
          proxies are not supported by Headscale.
        </>
      }
      footerActions={
        <>
          <LinkButton
            href="https://headscale.net/stable/"
            external
            size="xs"
            variant="secondary"
            icon={<ArrowSquareOutIcon />}
          >
            Docs
          </LinkButton>
          {installed && hs?.running ? (
            <Button
              size="xs"
              variant="secondary-destructive"
              onClick={() => setStop(true)}
            >
              Stop
            </Button>
          ) : null}
        </>
      }
    >
      {!installed || !hs?.running ? (
        <>
          <ol className="grid grid-cols-1 gap-3 py-4 sm:grid-cols-2">
            {STEPS.map((s, i) => (
              <li
                key={s.title}
                className="flex gap-3 rounded-lg bg-kumo-elevated p-3 ring-1 ring-kumo-hairline"
              >
                <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-kumo-contrast text-xs font-semibold text-kumo-inverse">
                  {i + 1}
                </span>
                <span className="min-w-0">
                  <span className="block text-sm font-medium">{s.title}</span>
                  <span className="block text-xs text-kumo-subtle text-pretty">
                    {s.body}
                  </span>
                </span>
              </li>
            ))}
          </ol>
          <SettingRow
            label="Install Headscale"
            description="Ports 80 and 443 on this Mac must be free."
            stack
          >
            <form
              className="grid grid-cols-1 gap-2 sm:grid-cols-3"
              onSubmit={(e) => {
                e.preventDefault();
                install.mutate(form);
              }}
            >
              <TextInput
                aria-label="Public hostname"
                placeholder="hs.example.com…"
                required
                value={form.hostname}
                spellCheck={false}
                autoComplete="off"
                onChange={(e) => setForm({ ...form, hostname: e.target.value })}
              />
              <TextInput
                aria-label="Email for Let's Encrypt"
                type="email"
                placeholder="you@example.com…"
                required
                value={form.acme_email}
                autoComplete="email"
                onChange={(e) =>
                  setForm({ ...form, acme_email: e.target.value })
                }
              />
              <TextInput
                aria-label="Device domain (MagicDNS)"
                placeholder="tail.internal"
                required
                value={form.base_domain}
                spellCheck={false}
                autoComplete="off"
                onChange={(e) =>
                  setForm({ ...form, base_domain: e.target.value })
                }
              />
              <Button
                type="submit"
                variant="primary"
                loading={install.isPending}
                className="justify-self-start sm:col-span-3"
              >
                {installed ? "Start Headscale" : "Install Headscale"}
              </Button>
            </form>
            {install.error ? (
              <Banner
                variant="error"
                title="Install failed"
                description={install.error.message}
              />
            ) : null}
          </SettingRow>
        </>
      ) : (
        <HeadscaleManage />
      )}
      {stop ? (
        <Confirm
          title="Stop Headscale?"
          body="Devices on this tailnet stay connected for a while but can’t log in or see changes until it’s back. Data is kept."
          action="Stop"
          pending={uninstall.isPending}
          onClose={() => setStop(false)}
          onConfirm={() =>
            uninstall.mutate(undefined, { onSettled: () => setStop(false) })
          }
        />
      ) : null}
    </SettingsGroup>
  );
}

function HeadscaleManage() {
  const { data: hs } = useHeadscale();
  const addUser = useHeadscaleUser();
  const mkKey = useHeadscaleKey();
  const joinSelf = useHeadscaleJoinSelf();
  const [user, setUser] = useState("");
  const [keyFor, setKeyFor] = useState<string>("");
  const [reusable, setReusable] = useState(false);
  const url = `https://${hs?.config?.hostname ?? ""}`;
  const users = hs?.users ?? [];
  const nodes = hs?.nodes ?? [];
  const key = (mkKey.data as { key?: string } | undefined)?.key;

  return (
    <>
      {hs?.reachable === false ? (
        <div className="py-4">
          <Banner
            variant="alert"
            title={`${url} isn’t reachable yet`}
            description="Check the DNS record and that ports 80/443 are forwarded to this Mac. The certificate is issued on first access."
          />
        </div>
      ) : null}
      <SettingRow
        label="Server"
        description="Point Tailscale apps here with “Use a custom control server”."
      >
        <ClipboardText text={url} />
      </SettingRow>
      <SettingRow
        label="Users"
        description={
          users.length
            ? users.map((u) => u.name).join(", ")
            : "No users yet. Create one per person."
        }
        stack
      >
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            addUser.mutate(user.trim().toLowerCase(), {
              onSuccess: () => setUser(""),
            });
          }}
        >
          <TextInput
            aria-label="New user name"
            className="sm:w-56"
            placeholder="alex…"
            value={user}
            spellCheck={false}
            autoComplete="off"
            onChange={(e) => setUser(e.target.value)}
          />
          <Button
            type="submit"
            size="sm"
            icon={<UserPlusIcon />}
            disabled={!user.trim()}
            loading={addUser.isPending}
          >
            Add User
          </Button>
        </form>
        {addUser.error ? (
          <p className="text-xs text-kumo-danger">{addUser.error.message}</p>
        ) : null}
      </SettingRow>
      {users.length ? (
        <SettingRow
          label="Add a device"
          description="Generate a key, then run the command on the device (Tailscale must be installed there)."
          stack
        >
          <div className="flex flex-wrap items-center gap-3">
            <Select
              aria-label="User"
              className="w-48"
              size="sm"
              value={keyFor}
              placeholder="Choose user…"
              onChange={setKeyFor}
              items={Object.fromEntries(
                users.map((u) => [String(u.id), u.name]),
              )}
            />
            <Switch
              size="sm"
              label="Reusable"
              checked={reusable}
              onCheckedChange={setReusable}
            />
            <Button
              size="sm"
              icon={<KeyIcon />}
              disabled={!keyFor}
              loading={mkKey.isPending}
              onClick={() =>
                mkKey.mutate({ user: Number(keyFor), reusable, hours: 24 })
              }
            >
              Generate Key
            </Button>
          </div>
          {key ? (
            <div className="flex flex-col gap-2">
              <ClipboardText
                text={`tailscale up --login-server ${url} --authkey ${key}`}
              />
              <p className="text-xs text-kumo-subtle">
                Valid for 24 hours{reusable ? ", reusable" : ", single use"}. On
                iOS/Android: Settings → Accounts → Use custom coordination
                server → {url}.
              </p>
              <Button
                size="sm"
                variant="secondary"
                className="self-start"
                loading={joinSelf.isPending}
                onClick={() => joinSelf.mutate(key)}
              >
                Also Join This Mac
              </Button>
              {joinSelf.error ? (
                <p className="text-xs text-kumo-danger">
                  {joinSelf.error.message}
                </p>
              ) : null}
            </div>
          ) : null}
        </SettingRow>
      ) : null}
      <SettingRow
        label="Devices"
        description={
          nodes.length
            ? `${nodes.length} device${nodes.length > 1 ? "s" : ""}`
            : "No devices have joined yet."
        }
        stack
      >
        {nodes.length ? (
          <ul className="divide-y divide-kumo-hairline rounded-lg ring-1 ring-kumo-hairline">
            {nodes.map((n) => (
              <li
                key={String(n.id)}
                className="flex items-center gap-3 px-3 py-2 text-sm"
              >
                <StatusDot
                  tone={n.online ? "success" : "neutral"}
                  live={n.online}
                  label={n.online ? "Online" : "Offline"}
                />
                <span className="flex-1 font-medium">
                  {n.given_name ?? n.name}
                </span>
                <span className="font-mono text-xs text-kumo-subtle">
                  {n.ip_addresses?.[0]}
                </span>
                <span className="text-xs text-kumo-subtle">{n.user?.name}</span>
                {typeof n.last_seen === "string" ? (
                  <span className="text-xs text-kumo-subtle">
                    {formatRelative(n.last_seen)}
                  </span>
                ) : null}
              </li>
            ))}
          </ul>
        ) : null}
      </SettingRow>
    </>
  );
}
