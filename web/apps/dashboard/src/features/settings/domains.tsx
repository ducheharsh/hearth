import { useState, type FormEvent } from "react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { SensitiveInput } from "@cloudflare/kumo/components/sensitive-input";
import { ArrowSquareOutIcon, GlobeIcon } from "@phosphor-icons/react";
import { Select, TextInput } from "@hearth/ui/components/form";
import {
  SettingRow,
  SettingsGroup,
  SettingsHeader,
} from "@hearth/ui/components/settings";
import {
  useCatalog,
  useCloudflare,
  useCloudflareConnect,
  useCloudflareDisconnect,
  useFunnel,
  useNgrok,
  useSettings,
  useTailscale,
} from "../../api/hooks";

export function DomainsSection() {
  return (
    <>
      <SettingsHeader
        title="Domains & Tunnels"
        description="Optional. Your own devices already reach everything privately. Use these to share apps on the internet."
      />
      <CloudflareGroup />
      <FunnelGroup />
      <NgrokGroup />
    </>
  );
}

function useAppItems() {
  const { data } = useCatalog();
  const items: Record<string, string> = { "": "Nothing (off)" };
  for (const a of data?.apps ?? []) if (a.installed) items[a.id] = a.name;
  return items;
}

function CloudflareGroup() {
  const cf = useCloudflare();
  const connect = useCloudflareConnect();
  const disconnect = useCloudflareDisconnect();
  const [token, setToken] = useState("");
  const [domain, setDomain] = useState("");
  const [confirm, setConfirm] = useState(false);
  const connected = Boolean(cf.data?.domain && cf.data.enabled);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    connect.mutate(
      { api_token: token, domain },
      { onSuccess: () => setToken("") },
    );
  };

  return (
    <SettingsGroup
      title="Your domain (Cloudflare)"
      description="Gives apps addresses like status.yourdomain.com, always behind your login. hearth creates the tunnel, routing and DNS for you."
      aside={
        connected ? (
          <Badge
            variant={cf.data?.connector_running ? "success" : "warning"}
            appearance="dot"
          >
            {cf.data?.connector_running ? "Live" : "Starting"}
          </Badge>
        ) : (
          <Badge variant="purple">Recommended</Badge>
        )
      }
      footer="Token permissions: Account › Cloudflare Tunnel › Edit, Zone › DNS › Edit, Zone › Zone › Read."
      footerActions={
        <LinkButton
          href="https://dash.cloudflare.com/profile/api-tokens"
          external
          size="xs"
          variant="secondary"
          icon={<ArrowSquareOutIcon />}
        >
          Create Token
        </LinkButton>
      }
    >
      {connected ? (
        <>
          <SettingRow
            label="Domain"
            description={`*.${cf.data?.domain} routes to this server.`}
          >
            <LinkButton
              href={`https://hearth.${cf.data?.domain}`}
              external
              size="sm"
              icon={<ArrowSquareOutIcon />}
            >
              Open hearth.{cf.data?.domain}
            </LinkButton>
          </SettingRow>
          <SettingRow
            label="Public apps"
            description={
              cf.data?.public_apps.length
                ? cf.data.public_apps.join(", ")
                : "None yet. Open an installed app’s details and turn on “Public on my domain”."
            }
          />
          <SettingRow
            label="Disconnect"
            description="Stop serving your domain; optionally delete the tunnel and DNS record."
          >
            <Button
              size="sm"
              variant="secondary-destructive"
              onClick={() => setConfirm(true)}
            >
              Disconnect
            </Button>
          </SettingRow>
        </>
      ) : (
        <SettingRow
          label="Connect a domain"
          description="The domain must already be on your Cloudflare account."
          stack
        >
          <form
            onSubmit={submit}
            className="grid grid-cols-1 gap-2 sm:grid-cols-2"
          >
            <TextInput
              aria-label="Domain"
              required
              placeholder="home.example.com…"
              spellCheck={false}
              autoComplete="off"
              value={domain}
              onChange={(e) => setDomain(e.target.value.toLowerCase())}
            />
            <SensitiveInput
              aria-label="API token"
              placeholder="API token…"
              value={token}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setToken(e.target.value)
              }
            />
            {connect.error ? (
              <Banner
                variant="error"
                title="Setup failed"
                description={connect.error.message}
                className="sm:col-span-2"
              />
            ) : null}
            <Button
              type="submit"
              size="sm"
              variant="primary"
              icon={<GlobeIcon />}
              loading={connect.isPending}
              disabled={!token || !domain}
              className="justify-self-start"
            >
              {connect.isPending ? "Setting Up…" : "Connect Domain"}
            </Button>
          </form>
        </SettingRow>
      )}
      {confirm ? (
        <Dialog.Root open onOpenChange={setConfirm}>
          <Dialog size="base" className="flex flex-col gap-4 p-6">
            <Dialog.Title className="text-lg font-semibold">
              Disconnect {cf.data?.domain}?
            </Dialog.Title>
            <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
              Apps stop being reachable on your domain. Keep the tunnel for
              later, or delete it and its DNS record.
            </Dialog.Description>
            <div className="flex flex-wrap justify-end gap-2">
              <Dialog.Close
                render={(p) => (
                  <Button {...p} variant="secondary">
                    Cancel
                  </Button>
                )}
              />
              <Button
                loading={disconnect.isPending}
                onClick={() =>
                  disconnect.mutate(
                    { delete: false },
                    { onSettled: () => setConfirm(false) },
                  )
                }
              >
                Stop, Keep Tunnel
              </Button>
              <Button
                variant="destructive"
                loading={disconnect.isPending}
                onClick={() =>
                  disconnect.mutate(
                    { delete: true },
                    { onSettled: () => setConfirm(false) },
                  )
                }
              >
                Delete Tunnel & DNS
              </Button>
            </div>
          </Dialog>
        </Dialog.Root>
      ) : null}
    </SettingsGroup>
  );
}

function FunnelGroup() {
  const settings = useSettings();
  const ts = useTailscale();
  const funnel = useFunnel();
  const items = useAppItems();
  const current = settings.data?.tunnels.funnel_app ?? "";
  const [app, setApp] = useState<string | null>(null);
  const value = app ?? current;
  return (
    <SettingsGroup
      title="Tailscale Funnel"
      description={`Share one app at https://${ts.data?.dns_name ?? "your-machine.ts.net"}. No domain needed.`}
      aside={
        current ? (
          <Badge variant="warning" appearance="dot">
            Public
          </Badge>
        ) : null
      }
      footer="The app must have its own login: the hearth login can’t cover Funnel links. Funnel must be allowed in your tailnet policy."
    >
      <SettingRow label="Shared app">
        <div className="flex gap-2">
          <Select
            aria-label="App to share with Funnel"
            className="w-52"
            value={value}
            items={items}
            onChange={setApp}
          />
          <Button
            size="sm"
            variant={value ? "primary" : "secondary"}
            loading={funnel.isPending}
            disabled={value === current}
            onClick={() =>
              funnel.mutate(value || null, { onSuccess: () => setApp(null) })
            }
          >
            Apply
          </Button>
        </div>
      </SettingRow>
      {funnel.error ? (
        <div className="py-3">
          <Banner
            variant="error"
            title="Funnel failed"
            description={funnel.error.message}
          />
        </div>
      ) : null}
    </SettingsGroup>
  );
}

function NgrokGroup() {
  const settings = useSettings();
  const ngrok = useNgrok();
  const items = useAppItems();
  const t = settings.data?.tunnels;
  const [app, setApp] = useState<string | null>(null);
  const [domain, setDomain] = useState<string | null>(null);
  const [token, setToken] = useState("");
  const value = app ?? t?.ngrok_app ?? "";
  const dom = domain ?? t?.ngrok_domain ?? "";
  return (
    <SettingsGroup
      title="ngrok"
      description="Share one app on your free ngrok static domain. Handy for webhooks."
      aside={
        t?.ngrok_app ? (
          <Badge variant="warning" appearance="dot">
            Public
          </Badge>
        ) : null
      }
      footer="The app must have its own login: the hearth login can’t cover ngrok links."
      footerActions={
        <LinkButton
          href="https://dashboard.ngrok.com/domains"
          external
          size="xs"
          variant="secondary"
          icon={<ArrowSquareOutIcon />}
        >
          Get a Domain
        </LinkButton>
      }
    >
      <SettingRow label="Shared app">
        <Select
          aria-label="App to share with ngrok"
          className="w-52"
          value={value}
          items={items}
          onChange={setApp}
        />
      </SettingRow>
      {value ? (
        <>
          <SettingRow label="Static domain" htmlFor="ngrok-domain">
            <TextInput
              id="ngrok-domain"
              className="sm:w-64"
              value={dom}
              placeholder="name.ngrok-free.app…"
              spellCheck={false}
              autoComplete="off"
              onChange={(e) => setDomain(e.target.value.toLowerCase())}
            />
          </SettingRow>
          <SettingRow
            label="Authtoken"
            description="Only needed the first time; stored privately on this machine."
          >
            <SensitiveInput
              aria-label="ngrok authtoken"
              className="sm:w-64"
              value={token}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                setToken(e.target.value)
              }
            />
          </SettingRow>
        </>
      ) : null}
      <SettingRow label={value ? "Start sharing" : "Stop sharing"}>
        <Button
          size="sm"
          variant={value ? "primary" : "secondary"}
          loading={ngrok.isPending}
          disabled={!value && !t?.ngrok_app}
          onClick={() =>
            ngrok.mutate(
              {
                app: value || null,
                domain: dom,
                authtoken: token || undefined,
              },
              {
                onSuccess: () => {
                  setApp(null);
                  setToken("");
                },
              },
            )
          }
        >
          {value ? "Start ngrok" : "Turn Off"}
        </Button>
      </SettingRow>
      {ngrok.error ? (
        <div className="py-3">
          <Banner
            variant="error"
            title="ngrok failed"
            description={ngrok.error.message}
          />
        </div>
      ) : null}
    </SettingsGroup>
  );
}
