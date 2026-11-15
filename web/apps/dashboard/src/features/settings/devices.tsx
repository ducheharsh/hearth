import { useState } from "react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import {
  DesktopIcon,
  DeviceMobileIcon,
  DeviceTabletIcon,
  SignOutIcon,
  type Icon,
} from "@phosphor-icons/react";
import { SettingsGroup } from "@hearth/ui/components/settings";
import { formatRelative } from "@hearth/ui/format";
import {
  useSessions,
  useSignOutAll,
  useSignOutSession,
  type DeviceSession,
} from "../../api/hooks";

function deviceIcon(device: string): Icon {
  if (/iPhone|Android/.test(device)) return DeviceMobileIcon;
  if (/iPad/.test(device)) return DeviceTabletIcon;
  return DesktopIcon;
}

/** Group sessions by user, keeping the newest-active order. */
function byUser(list: DeviceSession[]) {
  const groups = new Map<string, DeviceSession[]>();
  for (const s of list) groups.set(s.user, [...(groups.get(s.user) ?? []), s]);
  return [...groups.entries()];
}

function ConfirmEveryone({
  count,
  pending,
  onConfirm,
  onClose,
}: {
  count: number;
  pending: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="base" className="flex flex-col gap-4 p-6">
        <Dialog.Title className="text-lg font-semibold">
          Sign out everyone?
        </Dialog.Title>
        <Dialog.Description className="text-sm text-kumo-subtle text-pretty">
          All {count} {count === 1 ? "device" : "devices"}, including this one
          if you’re using hearth through the gateway, must sign in again (with
          2FA). Nobody’s login is changed.
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
            Sign Out Everyone
          </Button>
        </div>
      </Dialog>
    </Dialog.Root>
  );
}

/** Who is signed in where, with sign-out per device, per person, or for everyone. */
export function SignedInDevices() {
  const { data, error } = useSessions();
  const one = useSignOutSession();
  const all = useSignOutAll();
  const [confirm, setConfirm] = useState(false);
  const list = data?.sessions ?? [];
  const failure = error ?? one.error ?? all.error;

  return (
    <SettingsGroup
      title="Signed-in devices"
      description={
        data?.running === false
          ? "Turn on the gateway to see who’s signed in."
          : `${list.length} ${list.length === 1 ? "device" : "devices"} signed in. Signing a device out takes effect immediately; disabling a user or changing their password signs them out everywhere.`
      }
      footerActions={
        list.length ? (
          <Button
            size="xs"
            variant="secondary-destructive"
            icon={<SignOutIcon />}
            onClick={() => setConfirm(true)}
          >
            Sign Out Everyone
          </Button>
        ) : null
      }
    >
      {failure ? (
        <div className="py-3">
          <Banner
            variant="error"
            title="Couldn’t reach the session store"
            description={failure.message}
          />
        </div>
      ) : null}
      {data?.running && list.length === 0 ? (
        <p className="py-6 text-sm text-kumo-subtle">
          Nobody is signed in right now. A device shows up here once it opens an
          app or the dashboard after signing in.
        </p>
      ) : null}
      <div className="divide-y divide-kumo-hairline">
        {byUser(list).map(([user, sessions]) => (
          <section
            key={user}
            aria-label={`${sessions[0]?.display_name ?? user}’s devices`}
            className="flex flex-col gap-1 py-3"
          >
            <div className="flex items-center gap-3">
              <span className="min-w-0 flex-1 truncate text-sm font-medium">
                {sessions[0]?.display_name ?? user}{" "}
                <span className="font-normal text-kumo-subtle">
                  · {user} · {sessions.length}{" "}
                  {sessions.length === 1 ? "device" : "devices"}
                </span>
              </span>
              {sessions.length > 1 ? (
                <Button
                  size="xs"
                  variant="ghost"
                  loading={all.isPending && all.variables === user}
                  onClick={() => all.mutate(user)}
                >
                  Sign Out All
                </Button>
              ) : null}
            </div>
            <ul className="flex flex-col">
              {sessions.map((s) => {
                const I = deviceIcon(s.device);
                const where = [
                  s.node?.name,
                  s.apps.length ? `used ${s.apps.join(", ")}` : null,
                  `active ${formatRelative(s.last_seen)}`,
                ]
                  .filter(Boolean)
                  .join(" · ");
                return (
                  <li key={s.id} className="flex items-center gap-3 py-2">
                    <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-kumo-tint text-kumo-subtle">
                      <I size={18} aria-hidden="true" />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex flex-wrap items-center gap-2 text-sm">
                        {s.device}
                        {s.current ? (
                          <Badge variant="success" appearance="dot">
                            This device
                          </Badge>
                        ) : null}
                      </span>
                      <span className="block text-xs text-kumo-subtle text-pretty">
                        {where} · since {formatRelative(s.first_seen)}
                      </span>
                    </span>
                    <Button
                      size="sm"
                      variant="secondary"
                      icon={<SignOutIcon />}
                      loading={one.isPending && one.variables === s.id}
                      onClick={() => one.mutate(s.id)}
                      aria-label={`Sign out ${s.device} (${user})`}
                    >
                      Sign Out
                    </Button>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
      {confirm ? (
        <ConfirmEveryone
          count={list.length}
          pending={all.isPending}
          onClose={() => setConfirm(false)}
          onConfirm={() =>
            all.mutate(null, { onSuccess: () => setConfirm(false) })
          }
        />
      ) : null}
    </SettingsGroup>
  );
}
