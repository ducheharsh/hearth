import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button, LinkButton } from "@cloudflare/kumo/components/button";
import { SensitiveInput } from "@cloudflare/kumo/components/sensitive-input";
import { Switch } from "@cloudflare/kumo/components/switch";
import { ArrowSquareOutIcon, PaperPlaneTiltIcon } from "@phosphor-icons/react";
import { Select, TextInput } from "@hearth/ui/components/form";
import {
  SaveBar,
  SettingRow,
  SettingsGroup,
  SettingsHeader,
} from "@hearth/ui/components/settings";
import { api, post, put } from "../../api/client";

interface AlertSettings {
  min_level: "info" | "warning" | "error";
  ntfy: { enabled: boolean; server: string; topic: string };
  telegram: { enabled: boolean; chat_id: string };
  webhook: { enabled: boolean; url: string };
  watch_apps: boolean;
  watch_resources: boolean;
  watch_logins: boolean;
}
interface AlertsResponse {
  settings: AlertSettings;
  has_ntfy_token: boolean;
  has_telegram_token: boolean;
}

export function AlertsSection() {
  const { data, refetch } = useQuery({
    queryKey: ["alerts"],
    queryFn: () => api<AlertsResponse>("/alerts"),
  });
  const [s, setS] = useState<AlertSettings | null>(null);
  const [tokens, setTokens] = useState({ ntfy: "", telegram: "" });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [test, setTest] = useState<{ sent: number; errors: string[] } | null>(
    null,
  );
  const [testing, setTesting] = useState(false);
  useEffect(() => {
    if (data) setS(data.settings);
  }, [data]);
  if (!s || !data) return null;
  const dirty =
    JSON.stringify(s) !== JSON.stringify(data.settings) ||
    tokens.ntfy !== "" ||
    tokens.telegram !== "";

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await put("/alerts", {
        settings: s,
        ntfy_token: tokens.ntfy || undefined,
        telegram_bot_token: tokens.telegram || undefined,
      });
      setTokens({ ntfy: "", telegram: "" });
      await refetch();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  const sendTest = async () => {
    setTesting(true);
    setTest(null);
    try {
      setTest(await post<{ sent: number; errors: string[] }>("/alerts/test"));
    } catch (e) {
      setTest({ sent: 0, errors: [(e as Error).message] });
    } finally {
      setTesting(false);
    }
  };

  return (
    <>
      <SettingsHeader
        title="Alerts"
        description="Get a notification on your phone when something needs you: an app crashes, a drive disconnects, a backup fails, or someone fails to sign in."
      />

      <SettingsGroup
        title="Channels"
        description="Turn on one or more. ntfy is the easiest: install the ntfy app and subscribe to your topic."
        footerActions={
          <Button
            size="xs"
            icon={<PaperPlaneTiltIcon />}
            loading={testing}
            disabled={dirty}
            onClick={() => void sendTest()}
          >
            Send Test
          </Button>
        }
        footer={
          dirty ? (
            "Save changes before testing."
          ) : test ? (
            test.errors.length ? (
              <span className="text-kumo-danger">
                {test.errors.join(" · ")}
              </span>
            ) : (
              `Sent to ${test.sent} channel${test.sent === 1 ? "" : "s"}.`
            )
          ) : undefined
        }
      >
        <SettingRow
          label="ntfy"
          description="Push notifications via ntfy.sh or your own ntfy server. Pick a long, unguessable topic."
          stack
        >
          <div className="flex flex-col gap-2">
            <Switch
              label="Send to ntfy"
              checked={s.ntfy.enabled}
              onCheckedChange={(v) =>
                setS({ ...s, ntfy: { ...s.ntfy, enabled: v } })
              }
            />
            {s.ntfy.enabled ? (
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
                <TextInput
                  aria-label="ntfy server"
                  value={s.ntfy.server}
                  spellCheck={false}
                  autoComplete="off"
                  onChange={(e) =>
                    setS({ ...s, ntfy: { ...s.ntfy, server: e.target.value } })
                  }
                />
                <TextInput
                  aria-label="ntfy topic"
                  placeholder="hearth-7f3k9q…"
                  value={s.ntfy.topic}
                  spellCheck={false}
                  autoComplete="off"
                  onChange={(e) =>
                    setS({ ...s, ntfy: { ...s.ntfy, topic: e.target.value } })
                  }
                />
                <SensitiveInput
                  aria-label="ntfy access token (optional)"
                  placeholder={
                    data.has_ntfy_token
                      ? "Token saved"
                      : "Access token (optional)…"
                  }
                  value={tokens.ntfy}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setTokens({ ...tokens, ntfy: e.target.value })
                  }
                />
              </div>
            ) : null}
          </div>
        </SettingRow>
        <SettingRow
          label="Telegram"
          description="Create a bot with @BotFather, send it a message, then use your chat ID."
          stack
        >
          <div className="flex flex-col gap-2">
            <Switch
              label="Send to Telegram"
              checked={s.telegram.enabled}
              onCheckedChange={(v) =>
                setS({ ...s, telegram: { ...s.telegram, enabled: v } })
              }
            />
            {s.telegram.enabled ? (
              <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                <SensitiveInput
                  aria-label="Bot token"
                  placeholder={
                    data.has_telegram_token ? "Bot token saved" : "123456:ABC…"
                  }
                  value={tokens.telegram}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setTokens({ ...tokens, telegram: e.target.value })
                  }
                />
                <TextInput
                  aria-label="Chat ID"
                  placeholder="Chat ID…"
                  inputMode="numeric"
                  value={s.telegram.chat_id}
                  autoComplete="off"
                  onChange={(e) =>
                    setS({
                      ...s,
                      telegram: { ...s.telegram, chat_id: e.target.value },
                    })
                  }
                />
              </div>
            ) : null}
          </div>
        </SettingRow>
        <SettingRow
          label="Webhook"
          description="POSTs JSON (title, message, level, source) to any URL: Home Assistant, Discord bridges, your own scripts."
          stack
        >
          <div className="flex flex-col gap-2">
            <Switch
              label="Send to a webhook"
              checked={s.webhook.enabled}
              onCheckedChange={(v) =>
                setS({ ...s, webhook: { ...s.webhook, enabled: v } })
              }
            />
            {s.webhook.enabled ? (
              <TextInput
                aria-label="Webhook URL"
                placeholder="https://…"
                value={s.webhook.url}
                spellCheck={false}
                autoComplete="off"
                onChange={(e) =>
                  setS({ ...s, webhook: { ...s.webhook, url: e.target.value } })
                }
              />
            ) : null}
          </div>
        </SettingRow>
      </SettingsGroup>

      <SettingsGroup
        title="What to watch"
        footerActions={
          <LinkButton
            href="https://ntfy.sh/app"
            external
            size="xs"
            variant="secondary"
            icon={<ArrowSquareOutIcon />}
          >
            Get ntfy
          </LinkButton>
        }
      >
        <SettingRow
          label="Notify me about"
          description="Everything at this level or worse."
        >
          <Select
            aria-label="Minimum level"
            size="sm"
            className="w-44"
            value={s.min_level}
            onChange={(v) =>
              setS({ ...s, min_level: v as AlertSettings["min_level"] })
            }
            items={{
              error: "Only problems",
              warning: "Problems & warnings",
              info: "Everything",
            }}
          />
        </SettingRow>
        <SettingRow label="App crashes & failing health checks">
          <Switch
            checked={s.watch_apps}
            onCheckedChange={(v) => setS({ ...s, watch_apps: v })}
            aria-label="Watch apps"
          />
        </SettingRow>
        <SettingRow label="Low disk & memory pressure">
          <Switch
            checked={s.watch_resources}
            onCheckedChange={(v) => setS({ ...s, watch_resources: v })}
            aria-label="Watch resources"
          />
        </SettingRow>
        <SettingRow
          label="Failed sign-ins"
          description="Read from the login service while the gateway runs."
        >
          <Switch
            checked={s.watch_logins}
            onCheckedChange={(v) => setS({ ...s, watch_logins: v })}
            aria-label="Watch sign-ins"
          />
        </SettingRow>
      </SettingsGroup>
      {error ? (
        <Banner variant="error" title="Couldn’t save" description={error} />
      ) : null}
      <SaveBar
        visible={dirty}
        saving={saving}
        error={error}
        onDiscard={() => {
          setS(data.settings);
          setTokens({ ntfy: "", telegram: "" });
        }}
        onSave={() => void save()}
      />
    </>
  );
}
