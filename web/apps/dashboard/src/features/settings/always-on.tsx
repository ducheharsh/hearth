import { useState } from "react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Checkbox } from "@cloudflare/kumo/components/checkbox";
import { Switch } from "@cloudflare/kumo/components/switch";
import { ArrowsClockwiseIcon, LightningIcon } from "@phosphor-icons/react";
import {
  SettingRow,
  SettingsGroup,
  SettingsHeader,
} from "@hearth/ui/components/settings";
import { StatusDot } from "@hearth/ui/components/status-dot";
import { formatRelative } from "@hearth/ui/format";
import {
  useAlwaysOn,
  useAlwaysOnUpdate,
  useApplyPower,
  useConnectivityCheck,
  useWifiPower,
} from "../../api/hooks";
import type { AlwaysOn, PowerStatus } from "../../api/types";

type Tone = "success" | "warning" | "neutral";

function State({ tone, children }: { tone: Tone; children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-sm">
      <StatusDot tone={tone} label={String(children)} />
      {children}
    </span>
  );
}

const minutes = (m: number | null) =>
  m == null ? "Unknown" : m === 0 ? "Never" : `After ${m} min`;

export function AlwaysOnSection() {
  const { data } = useAlwaysOn();
  return (
    <>
      <SettingsHeader
        title="Always On"
        description="Keep this Mac awake, powered and online so your apps are there whenever you reach for them."
      />
      {data ? (
        <>
          <StayAwake data={data} />
          {data.power ? <Power power={data.power} /> : null}
          <Connectivity data={data} />
        </>
      ) : null}
    </>
  );
}

function StayAwake({ data }: { data: AlwaysOn }) {
  const update = useAlwaysOnUpdate();
  const p = data.power;
  const on = data.settings.keep_awake;
  const status: [Tone, string] = !on
    ? ["neutral", "Off"]
    : !data.holding
      ? ["warning", "Starting…"]
      : p && !p.on_ac
        ? ["warning", "Paused on battery"]
        : ["success", "Keeping this Mac awake"];
  return (
    <SettingsGroup
      title="Stay awake"
      description="hearth asks macOS not to sleep while it runs, like the caffeinate command. macOS only honours this on power, so a laptop on battery still sleeps to save it."
      aside={
        <Switch
          checked={on}
          disabled={update.isPending}
          onCheckedChange={(v) => update.mutate({ keep_awake: v })}
          aria-label="Keep this Mac awake"
        />
      }
    >
      <SettingRow label="Status" description="The display can still turn off.">
        <State tone={status[0]}>{status[1]}</State>
      </SettingRow>
      {p?.battery != null ? (
        <SettingRow
          label="Power source"
          description="For a home server, leave it plugged in."
        >
          <State tone={p.on_ac ? "success" : "warning"}>
            {p.on_ac ? "Charger" : "Battery"} · {p.battery}%
          </State>
        </SettingRow>
      ) : null}
    </SettingsGroup>
  );
}

function Power({ power: p }: { power: PowerStatus }) {
  const apply = useApplyPower();
  const [lid, setLid] = useState(p.sleep_disabled);
  const laptop = p.battery != null;
  const good =
    p.sleep_on_ac === 0 &&
    p.wake_on_lan === true &&
    p.restart_after_power_loss !== false &&
    (!laptop || p.sleep_disabled === lid);
  const yesNo = (v: boolean | null, unsupported: string) =>
    v == null ? (
      <State tone="neutral">{unsupported}</State>
    ) : (
      <State tone={v ? "success" : "warning"}>{v ? "On" : "Off"}</State>
    );
  return (
    <SettingsGroup
      title="Power"
      description="macOS settings that decide whether this Mac comes back by itself. Changing them needs your Mac password, which you type into macOS’s own prompt on this Mac."
      footerActions={
        <Button
          size="xs"
          variant={good ? "secondary" : "primary"}
          icon={<LightningIcon />}
          loading={apply.isPending}
          onClick={() => apply.mutate({ lid_closed: lid })}
        >
          {good ? "Apply Again" : "Apply Server Settings"}
        </Button>
      }
      footer={
        apply.isPending
          ? "Waiting for the password prompt on this Mac…"
          : "Never sleep on power, wake for network access, restart after a power failure."
      }
    >
      <SettingRow
        label="Sleep on power"
        description="When the Mac sleeps while plugged in."
      >
        <State tone={p.sleep_on_ac === 0 ? "success" : "warning"}>
          {minutes(p.sleep_on_ac)}
        </State>
      </SettingRow>
      <SettingRow
        label="Wake for network access"
        description="Lets other devices wake it if it does sleep."
      >
        {yesNo(p.wake_on_lan, "Not supported")}
      </SettingRow>
      <SettingRow
        label="Restart after a power failure"
        description={
          laptop
            ? "Laptops ride out power cuts on their battery instead."
            : "Starts up by itself when power returns."
        }
      >
        {yesNo(p.restart_after_power_loss, "Not on laptops")}
      </SettingRow>
      {laptop ? (
        <SettingRow
          label="Stay awake with the lid closed"
          description="Sleep is turned off completely, even on battery and with the lid shut. Use it only on a laptop that stays on its charger with room to breathe."
        >
          <span className="flex items-center gap-3">
            <Badge variant={p.sleep_disabled ? "success" : "secondary"}>
              {p.sleep_disabled ? "On now" : "Off now"}
            </Badge>
            <Checkbox
              label="Include"
              checked={lid}
              onCheckedChange={(v) => setLid(v === true)}
            />
          </span>
        </SettingRow>
      ) : null}
      {apply.error ? (
        <div className="py-3">
          <Banner
            variant="error"
            title="Power settings weren’t changed"
            description={apply.error.message}
          />
        </div>
      ) : null}
    </SettingsGroup>
  );
}

function Connectivity({ data }: { data: AlwaysOn }) {
  const update = useAlwaysOnUpdate();
  const check = useConnectivityCheck();
  const wifi = useWifiPower();
  const { connectivity: c, network: n, settings: s } = data;
  const link = n?.primary;
  const recent = c.outages.slice(-3).reverse();
  return (
    <SettingsGroup
      title="Connectivity"
      description="hearth checks the internet every 30 seconds, tells you when it drops and comes back, and fixes what it can by itself."
      footerActions={
        <Button
          size="xs"
          variant="secondary"
          icon={<ArrowsClockwiseIcon />}
          loading={check.isPending}
          onClick={() => check.mutate()}
        >
          Check Now
        </Button>
      }
    >
      <SettingRow
        label="Internet"
        description={
          c.checked_at
            ? `Checked ${formatRelative(c.checked_at)}`
            : "Not checked yet."
        }
      >
        {c.online == null ? (
          <State tone="neutral">Unknown</State>
        ) : c.online ? (
          <State tone="success">Online · {c.latency_ms ?? "?"} ms</State>
        ) : (
          <State tone="warning">Offline</State>
        )}
      </SettingRow>
      <SettingRow
        label="Connection"
        description={
          link?.kind === "wifi" && n?.has_ethernet
            ? "Wi-Fi works, but a cable is steadier for a server: plug in Ethernet if you can."
            : link
              ? `${link.port} (${link.device})${link.gateway ? ` via ${link.gateway}` : ""}`
              : "No network."
        }
      >
        {link ? (
          <State tone="success">
            {link.kind === "wifi"
              ? "Wi-Fi"
              : link.kind === "ethernet"
                ? "Ethernet"
                : link.port}
            {link.ip ? ` · ${link.ip}` : ""}
          </State>
        ) : (
          <State tone="warning">Disconnected</State>
        )}
      </SettingRow>
      {n?.wifi_device ? (
        <SettingRow label="Wi-Fi" description="The Mac’s Wi-Fi radio.">
          <span className="flex items-center gap-3">
            <State tone={n.wifi_on ? "success" : "warning"}>
              {n.wifi_on ? "On" : "Off"}
            </State>
            {!n.wifi_on ? (
              <Button
                size="xs"
                loading={wifi.isPending}
                onClick={() => wifi.mutate(true)}
              >
                Turn On
              </Button>
            ) : null}
          </span>
        </SettingRow>
      ) : null}
      <SettingRow
        label="Watch the connection"
        description="Alert when the internet drops and when it’s back (through your alert channels)."
      >
        <Switch
          checked={s.watch_connectivity}
          disabled={update.isPending}
          onCheckedChange={(v) => update.mutate({ watch_connectivity: v })}
          aria-label="Watch the connection"
        />
      </SettingRow>
      <SettingRow
        label="Fix problems automatically"
        description="Turn Wi-Fi back on if it was switched off, and reconnect Tailscale after an outage."
      >
        <Switch
          checked={s.auto_repair}
          disabled={update.isPending}
          onCheckedChange={(v) => update.mutate({ auto_repair: v })}
          aria-label="Fix problems automatically"
        />
      </SettingRow>
      <SettingRow
        label="Recent outages"
        description="Since hearth last started."
      >
        {recent.length === 0 ? (
          <span className="text-sm text-kumo-subtle">None</span>
        ) : (
          <ul className="flex flex-col items-end gap-0.5 text-sm">
            {recent.map((o) => (
              <li key={o.start}>
                {formatRelative(o.start)} ·{" "}
                {o.end
                  ? `${Math.max(1, Math.round((o.end - o.start) / 60))} min`
                  : "ongoing"}
              </li>
            ))}
          </ul>
        )}
      </SettingRow>
    </SettingsGroup>
  );
}
