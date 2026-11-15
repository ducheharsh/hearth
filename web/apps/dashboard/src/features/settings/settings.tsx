import { useEffect, useState } from "react";
import { motion } from "motion/react";
import { Button } from "@cloudflare/kumo/components/button";
import { Switch } from "@cloudflare/kumo/components/switch";
import {
  CheckCircleIcon,
  CircleDashedIcon,
  CircleIcon,
  CloudIcon,
  GaugeIcon,
  GearSixIcon,
  ListChecksIcon,
  LockKeyIcon,
  NetworkIcon,
  WarningCircleIcon,
  WrenchIcon,
} from "@phosphor-icons/react";
import { SearchSelect, TextInput } from "@hearth/ui/components/form";
import { SaveBar, SettingRow, SettingsGroup, SettingsHeader, SettingsLayout, type SettingsNavItem } from "@hearth/ui/components/settings";
import { springSoft } from "@hearth/ui/motion";
import { useSaveSettings, useSettings, useSetup } from "../../api/hooks";
import type { Settings as SettingsT } from "../../api/types";
import { useUrlParam } from "../../lib/url-state";
import { AccessSection } from "./access";
import { DomainsSection } from "./domains";
import { NetworkSection } from "./network";

type SectionId = "setup" | "general" | "network" | "access" | "domains" | "capacity" | "advanced";

const ZONES = (() => {
  try {
    return Intl.supportedValuesOf("timeZone");
  } catch {
    return ["UTC"];
  }
})();

/** Draft of the editable settings document, with a save bar when it diverges. */
function useDraft() {
  const { data } = useSettings();
  const save = useSaveSettings();
  const [draft, setDraft] = useState<SettingsT | null>(null);
  useEffect(() => {
    if (data) setDraft(data);
  }, [data]);
  const dirty = Boolean(data && draft && JSON.stringify(data) !== JSON.stringify(draft));
  return { data, draft, setDraft, dirty, save };
}

export function Settings() {
  const [section, setSection] = useUrlParam<SectionId>("section", "setup");
  const setup = useSetup();
  const todo = setup.data?.filter((c) => c.state === "todo").length ?? 0;
  const { data, draft, setDraft, dirty, save } = useDraft();

  const items: SettingsNavItem[] = [
    { id: "setup", label: "Setup", icon: ListChecksIcon, badge: todo > 0 ? todo : undefined },
    { id: "general", label: "General", icon: GearSixIcon },
    { id: "network", label: "Network", icon: NetworkIcon },
    { id: "access", label: "Logins & Gateway", icon: LockKeyIcon },
    { id: "domains", label: "Domains & Tunnels", icon: CloudIcon },
    { id: "capacity", label: "Capacity", icon: GaugeIcon },
    { id: "advanced", label: "Advanced", icon: WrenchIcon },
  ];

  const set = <K extends keyof SettingsT>(k: K, v: SettingsT[K]) => draft && setDraft({ ...draft, [k]: v });

  return (
    <>
      <SettingsLayout items={items} active={section} onSelect={(id) => setSection(id as SectionId)}>
        {section === "setup" ? <SetupSection onGo={(s) => setSection(s as SectionId)} /> : null}
        {section === "network" ? <NetworkSection /> : null}
        {section === "access" ? <AccessSection /> : null}
        {section === "domains" ? <DomainsSection /> : null}
        {draft && section === "general" ? (
          <>
            <SettingsHeader title="General" description="How this server identifies itself." />
            <SettingsGroup title="Identity">
              <SettingRow label="Server name" description="Shown in the header and the browser tab." htmlFor="display-name">
                <TextInput id="display-name" className="sm:w-64" value={draft.display_name} autoComplete="off" onChange={(e) => set("display_name", e.target.value)} />
              </SettingRow>
              <SettingRow label="Time zone" description="Passed to every app as TZ, so logs and schedules use local time.">
                <SearchSelect
                  aria-label="Time zone"
                  className="sm:w-64"
                  value={draft.tz}
                  options={ZONES.includes(draft.tz) ? ZONES : [draft.tz, ...ZONES]}
                  onChange={(v) => v && set("tz", v)}
                  placeholder="Search time zones…"
                />
              </SettingRow>
            </SettingsGroup>
            <SettingsGroup title="Directory" description="Discover and the leaderboards use the community awesome-selfhosted list.">
              <SettingRow label="Sync daily" description="Downloads ~20 MB once, then small updates. Turn off for a fully offline server.">
                <Switch checked={draft.discover_enabled} onCheckedChange={(v) => set("discover_enabled", v)} aria-label="Sync directory daily" />
              </SettingRow>
            </SettingsGroup>
          </>
        ) : null}
        {draft && section === "capacity" ? (
          <>
            <SettingsHeader title="Capacity" description="hearth refuses or warns about installs that would push this machine past these limits." />
            <SettingsGroup title="Memory" footer="An 8 GB Mac needs ~1.5 GB free for macOS and native apps like Ollama.">
              <SettingRow label="Always keep free" description="Installs that would go below this are refused.">
                <UnitInput value={draft.capacity.mem_reserve_mb} unit="MB" onChange={(v) => set("capacity", { ...draft.capacity, mem_reserve_mb: v })} label="RAM reserve" />
              </SettingRow>
              <SettingRow label="Warn when tighter than" description="Extra headroom above the reserve before an install is marked “Tight”.">
                <UnitInput value={draft.capacity.mem_warn_headroom_mb} unit="MB" onChange={(v) => set("capacity", { ...draft.capacity, mem_warn_headroom_mb: v })} label="RAM warning headroom" />
              </SettingRow>
            </SettingsGroup>
            <SettingsGroup title="Disk & drives">
              <SettingRow label="Internal disk floor" description="Never let the internal disk drop below this.">
                <UnitInput value={draft.capacity.disk_reserve_gb} unit="GB" onChange={(v) => set("capacity", { ...draft.capacity, disk_reserve_gb: v })} label="Disk reserve" />
              </SettingRow>
              <SettingRow label="Pause apps when their drive is unplugged" description="They start again automatically when the same drive returns.">
                <Switch checked={draft.auto_pause_on_disconnect} onCheckedChange={(v) => set("auto_pause_on_disconnect", v)} aria-label="Auto pause" />
              </SettingRow>
            </SettingsGroup>
          </>
        ) : null}
        {draft && section === "advanced" ? (
          <>
            <SettingsHeader title="Advanced" />
            <SettingsGroup title="Lock down the dashboard" tone={draft.require_remote_user ? "danger" : "default"}>
              <SettingRow
                label="Only allow access through the login gateway"
                description="Blocks direct http://127.0.0.1:9000 access; use https://<machine>:9443 instead. Turn on only after the gateway works."
              >
                <Switch checked={draft.require_remote_user} onCheckedChange={(v) => set("require_remote_user", v)} aria-label="Require gateway login" />
              </SettingRow>
            </SettingsGroup>
          </>
        ) : null}
      </SettingsLayout>
      <SaveBar
        visible={dirty}
        saving={save.isPending}
        error={save.error?.message ?? null}
        onDiscard={() => data && setDraft(data)}
        onSave={() => draft && save.mutate(draft)}
      />
    </>
  );
}

function UnitInput({ value, unit, onChange, label }: { value: number; unit: string; onChange: (v: number) => void; label: string }) {
  return (
    <span className="inline-flex items-center gap-2">
      <TextInput aria-label={label} inputMode="numeric" className="w-28 text-right tabular-nums" value={String(value)} onChange={(e) => onChange(Number(e.target.value.replace(/\D/g, "")) || 0)} />
      <span className="text-xs text-kumo-subtle">{unit}</span>
    </span>
  );
}

const STATE_ICON = {
  done: <CheckCircleIcon size={20} weight="fill" className="text-kumo-success" aria-hidden="true" />,
  warn: <WarningCircleIcon size={20} weight="fill" className="text-kumo-warning" aria-hidden="true" />,
  todo: <CircleIcon size={20} className="text-kumo-inactive" aria-hidden="true" />,
  optional: <CircleDashedIcon size={20} className="text-kumo-inactive" aria-hidden="true" />,
} as const;

/** Where each checklist item is fixed. */
const FIX: Record<string, { tab?: string; section?: string }> = {
  docker: { tab: "containers" },
  tailscale: { section: "network" },
  https: { section: "network" },
  vault: { tab: "storage" },
  login: { section: "access" },
  gateway: { section: "access" },
  domain: { section: "domains" },
};

function SetupSection({ onGo }: { onGo: (section: string) => void }) {
  const { data } = useSetup();
  const [, setTab] = useUrlParam<string>("tab", "overview");
  if (!data) return null;
  const required = data.filter((c) => c.state !== "optional");
  const done = required.filter((c) => c.state === "done").length;
  return (
    <>
      <SettingsHeader
        title="Setup"
        description={done === required.length ? "Everything is set up. Your server is private, protected and reachable from your devices." : "A few steps turn this Mac into a private, protected server. Do them in order."}
      />
      <SettingsGroup
        title={`${done} of ${required.length} complete`}
        aside={
          <span className="text-2xl font-semibold tabular-nums">
            {Math.round((done / Math.max(required.length, 1)) * 100)}
            <span className="text-sm text-kumo-subtle">%</span>
          </span>
        }
      >
        <div className="py-4">
          <div className="h-2 overflow-hidden rounded-full bg-kumo-fill" role="progressbar" aria-valuenow={done} aria-valuemin={0} aria-valuemax={required.length} aria-label="Setup progress">
            <motion.div className="h-full origin-left rounded-full bg-kumo-success" initial={{ scaleX: 0 }} animate={{ scaleX: done / Math.max(required.length, 1) }} transition={springSoft} />
          </div>
        </div>
        {data.map((c, i) => {
          const fix = FIX[c.id];
          return (
            <SettingRow
              key={c.id}
              label={`${i + 1}. ${c.title}`}
              description={
                <span className="flex items-start gap-2">
                  {STATE_ICON[c.state]}
                  <span className="pt-0.5">{c.detail}</span>
                </span>
              }
            >
              {c.state !== "done" && fix ? (
                <Button size="sm" variant={c.state === "todo" ? "primary" : "secondary"} onClick={() => (fix.tab ? setTab(fix.tab) : onGo(fix.section ?? "setup"))}>
                  {c.state === "optional" ? "Set Up" : "Fix"}
                </Button>
              ) : null}
            </SettingRow>
          );
        })}
      </SettingsGroup>
    </>
  );
}
