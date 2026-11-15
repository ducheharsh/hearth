import { useMemo, useState, type FormEvent } from "react";
import { Badge } from "@cloudflare/kumo/components/badge";
import { Banner } from "@cloudflare/kumo/components/banner";
import { Button } from "@cloudflare/kumo/components/button";
import { Checkbox } from "@cloudflare/kumo/components/checkbox";
import { Dialog } from "@cloudflare/kumo/components/dialog";
import { Switch } from "@cloudflare/kumo/components/switch";
import {
  CrownSimpleIcon,
  DeviceMobileIcon,
  PencilSimpleIcon,
  UserIcon,
  UserPlusIcon,
  WarningIcon,
  XIcon,
} from "@phosphor-icons/react";
import { AppLogo } from "@hearth/ui/components/app-logo";
import { DIALOG_FRAME, DialogFrame } from "@hearth/ui/components/dialog-frame";
import { Field, TextInput } from "@hearth/ui/components/form";
import { SettingsGroup, SettingsHeader } from "@hearth/ui/components/settings";
import {
  useAddUser,
  useCatalog,
  useDeleteUser,
  useEditUser,
  useTotpRemove,
  useUsers,
} from "../../api/hooks";
import type {
  CatalogApp,
  GatewayUser,
  UserRole as Role,
} from "../../api/types";
import { SignedInDevices } from "./devices";
import { TwoFactorDialog } from "./two-factor";

const ROLES: {
  role: Role;
  title: string;
  blurb: string;
  icon: typeof UserIcon;
}[] = [
  {
    role: "admin",
    title: "Admin",
    blurb: "Every app, plus this dashboard: settings, users and installs.",
    icon: CrownSimpleIcon,
  },
  {
    role: "member",
    title: "Member",
    blurb: "Only the apps you choose. Can’t open this dashboard.",
    icon: UserIcon,
  },
];

function initials(name: string) {
  return name
    .split(/\s+/)
    .map((p) => p[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
}

function Avatar({ user }: { user: GatewayUser }) {
  return (
    <span
      aria-hidden="true"
      className={`grid size-9 shrink-0 place-items-center rounded-full text-xs font-semibold ${user.disabled ? "bg-kumo-fill text-kumo-subtle" : user.role === "admin" ? "bg-kumo-brand/15 text-kumo-brand" : "bg-kumo-tint text-kumo-default"}`}
    >
      {initials(user.display_name || user.username)}
    </span>
  );
}

function accessText(u: GatewayUser, apps: CatalogApp[]) {
  if (u.role === "admin") return "Everything, including this dashboard";
  if (u.apps == null) return "All apps";
  if (u.apps.length === 0) return "No apps yet";
  const names = u.apps.map((id) => apps.find((a) => a.id === id)?.name ?? id);
  return names.length <= 3
    ? names.join(", ")
    : `${names.slice(0, 2).join(", ")} and ${names.length - 2} more`;
}

export function UsersSection() {
  const users = useUsers();
  const catalog = useCatalog();
  const installed = useMemo(
    () => catalog.data?.apps.filter((a) => a.installed) ?? [],
    [catalog.data],
  );
  const [editing, setEditing] = useState<GatewayUser | "new" | null>(null);
  const list = users.data ?? [];
  const admins = list.filter((u) => u.role === "admin" && !u.disabled).length;

  return (
    <>
      <SettingsHeader
        title="Users"
        description="Everyone who can sign in. Admins manage hearth; members only see the apps you give them. Changes apply right away."
      />

      <SettingsGroup
        title="People"
        description={`${list.length} ${list.length === 1 ? "login" : "logins"} · ${admins} active ${admins === 1 ? "admin" : "admins"}`}
        aside={
          <Button
            size="sm"
            variant="primary"
            icon={<UserPlusIcon />}
            onClick={() => setEditing("new")}
          >
            Add User
          </Button>
        }
      >
        {list.length === 0 ? (
          <p className="py-6 text-sm text-kumo-subtle">
            No logins yet. Add yourself first: the first user is an admin.
          </p>
        ) : (
          <ul className="divide-y divide-kumo-hairline">
            {list.map((u) => (
              <li key={u.username} className="flex items-center gap-3 py-3">
                <Avatar user={u} />
                <span className="min-w-0 flex-1">
                  <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <span
                      className={`truncate text-sm font-medium ${u.disabled ? "text-kumo-subtle line-through" : ""}`}
                    >
                      {u.display_name}
                    </span>
                    <Badge
                      variant={u.role === "admin" ? "primary" : "secondary"}
                    >
                      {u.role === "admin" ? "Admin" : "Member"}
                    </Badge>
                    {u.totp ? (
                      <Badge variant="success" appearance="dot">
                        2FA
                      </Badge>
                    ) : null}
                    {u.disabled ? (
                      <Badge variant="warning">Disabled</Badge>
                    ) : null}
                  </span>
                  <span className="block truncate text-xs text-kumo-subtle">
                    {u.username} · {accessText(u, installed)}
                  </span>
                </span>
                <Button
                  size="sm"
                  variant="secondary"
                  icon={<PencilSimpleIcon />}
                  onClick={() => setEditing(u)}
                  aria-label={`Edit ${u.display_name}`}
                >
                  Edit
                </Button>
              </li>
            ))}
          </ul>
        )}
      </SettingsGroup>

      <SignedInDevices />

      <AppAccess users={list} apps={installed} />

      {editing ? (
        <UserDialog
          key={editing === "new" ? "new" : editing.username}
          user={editing === "new" ? null : editing}
          first={list.length === 0}
          apps={installed}
          onClose={() => setEditing(null)}
        />
      ) : null}
    </>
  );
}

/** Per app: who can open it. */
function AppAccess({
  users,
  apps,
}: {
  users: GatewayUser[];
  apps: CatalogApp[];
}) {
  if (apps.length === 0 || users.length === 0) return null;
  const members = users.filter((u) => u.role === "member" && !u.disabled);
  return (
    <SettingsGroup
      title="App access"
      description="Who can open each installed app after signing in. Admins can open all of them."
    >
      <div className="divide-y divide-kumo-hairline">
        {apps.map((a) => {
          const who = members.filter(
            (m) => m.apps == null || m.apps.includes(a.id),
          );
          const everyone = who.length === members.length;
          return (
            <div key={a.id} className="flex items-center gap-3 py-3">
              <AppLogo
                src={a.icon}
                name={a.name}
                category={a.category}
                size="sm"
              />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium">
                  {a.name}
                </span>
                {a.protected === false ? (
                  <span className="block text-xs text-kumo-subtle">
                    Public: no sign-in, so access can’t be limited.
                  </span>
                ) : null}
              </span>
              <span className="max-w-[55%] text-right text-sm text-kumo-subtle text-pretty">
                {everyone
                  ? members.length
                    ? "Everyone"
                    : "Admins"
                  : who.length
                    ? `Admins, ${who.map((w) => w.display_name).join(", ")}`
                    : "Admins only"}
              </span>
            </div>
          );
        })}
      </div>
    </SettingsGroup>
  );
}

function UserDialog({
  user,
  first,
  apps,
  onClose,
}: {
  user: GatewayUser | null;
  first: boolean;
  apps: CatalogApp[];
  onClose: () => void;
}) {
  const add = useAddUser();
  const edit = useEditUser();
  const remove = useDeleteUser();
  const totpRemove = useTotpRemove();
  const isNew = user == null;
  const [f, setF] = useState({
    username: user?.username ?? "",
    display_name: user?.display_name ?? "",
    email: user?.email ?? "",
    password: "",
    role: (user?.role ?? (first ? "admin" : "member")) as Role,
    disabled: user?.disabled ?? false,
    allApps: user ? user.apps == null : true,
    apps: new Set(user?.apps ?? []),
  });
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [enroll, setEnroll] = useState(false);
  const busy = add.isPending || edit.isPending;
  const error = add.error ?? edit.error ?? remove.error ?? totpRemove.error;

  const submit = (e: FormEvent) => {
    e.preventDefault();
    const apps = f.role === "admin" || f.allApps ? null : [...f.apps];
    if (isNew) {
      add.mutate(
        {
          username: f.username.trim(),
          display_name: f.display_name,
          email: f.email,
          password: f.password,
          role: f.role,
          apps,
        },
        { onSuccess: onClose },
      );
    } else {
      edit.mutate(
        {
          username: user.username,
          display_name: f.display_name,
          email: f.email,
          role: f.role,
          disabled: f.disabled,
          apps,
          password: f.password || undefined,
        },
        { onSuccess: onClose },
      );
    }
  };
  const toggleApp = (id: string, on: boolean) =>
    setF((x) => {
      const s = new Set(x.apps);
      if (on) s.add(id);
      else s.delete(id);
      return { ...x, apps: s };
    });

  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog size="lg" className={DIALOG_FRAME}>
        <form onSubmit={submit} className="contents">
          <DialogFrame
            header={
              <div className="flex items-start justify-between gap-3">
                <div>
                  <Dialog.Title className="text-lg font-semibold">
                    {isNew ? "Add a user" : `Edit ${user.display_name}`}
                  </Dialog.Title>
                  <Dialog.Description className="text-sm text-kumo-subtle">
                    {isNew
                      ? "They’ll sign in with this username and password, then can add two-factor."
                      : `Signs in as ${user.username}.`}
                  </Dialog.Description>
                </div>
                <Dialog.Close
                  render={(p) => (
                    <Button
                      {...p}
                      variant="ghost"
                      shape="square"
                      icon={<XIcon />}
                      aria-label="Close"
                    />
                  )}
                />
              </div>
            }
            footer={
              <>
                <Dialog.Close
                  render={(p) => (
                    <Button {...p} variant="secondary">
                      Cancel
                    </Button>
                  )}
                />
                <Button type="submit" variant="primary" loading={busy}>
                  {isNew ? "Add User" : "Save Changes"}
                </Button>
              </>
            }
          >
            <div className="flex flex-col gap-6">
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <Field label="Name">
                  {(id) => (
                    <TextInput
                      id={id}
                      autoComplete="name"
                      placeholder="Alex Doe…"
                      value={f.display_name}
                      onChange={(e) =>
                        setF({ ...f, display_name: e.target.value })
                      }
                    />
                  )}
                </Field>
                <Field label="Email" hint="Used to set up two-factor.">
                  {(id) => (
                    <TextInput
                      id={id}
                      type="email"
                      required
                      autoComplete="email"
                      spellCheck={false}
                      placeholder="alex@example.com…"
                      value={f.email}
                      onChange={(e) => setF({ ...f, email: e.target.value })}
                    />
                  )}
                </Field>
                {isNew ? (
                  <Field
                    label="Username"
                    hint="Lowercase letters, numbers, . _ -"
                  >
                    {(id) => (
                      <TextInput
                        id={id}
                        required
                        autoComplete="username"
                        spellCheck={false}
                        placeholder="alex…"
                        value={f.username}
                        onChange={(e) =>
                          setF({ ...f, username: e.target.value.toLowerCase() })
                        }
                      />
                    )}
                  </Field>
                ) : null}
                <Field
                  label={isNew ? "Password" : "New password"}
                  hint={
                    isNew
                      ? "At least 12 characters."
                      : "Leave empty to keep the current password."
                  }
                >
                  {(id) => (
                    <TextInput
                      id={id}
                      type="password"
                      required={isNew}
                      minLength={12}
                      autoComplete="new-password"
                      placeholder={isNew ? "12+ characters…" : "••••••••••••"}
                      value={f.password}
                      onChange={(e) => setF({ ...f, password: e.target.value })}
                    />
                  )}
                </Field>
              </div>

              <fieldset className="flex flex-col gap-2">
                <legend className="mb-2 text-sm font-medium">Role</legend>
                <div
                  role="radiogroup"
                  aria-label="Role"
                  className="grid grid-cols-1 gap-2 sm:grid-cols-2"
                >
                  {ROLES.map((r) => {
                    const on = f.role === r.role;
                    const I = r.icon;
                    return (
                      <button
                        key={r.role}
                        type="button"
                        role="radio"
                        aria-checked={on}
                        onClick={() => setF({ ...f, role: r.role })}
                        className={`flex items-start gap-3 rounded-xl p-3 text-left ring-1 transition-[box-shadow,background-color] ${on ? "bg-kumo-tint ring-kumo-brand" : "ring-kumo-hairline hover:bg-kumo-tint"}`}
                      >
                        <I
                          size={20}
                          weight="duotone"
                          aria-hidden="true"
                          className="mt-0.5 shrink-0"
                        />
                        <span className="min-w-0">
                          <span className="block text-sm font-medium">
                            {r.title}
                          </span>
                          <span className="block text-xs text-kumo-subtle text-pretty">
                            {r.blurb}
                          </span>
                        </span>
                      </button>
                    );
                  })}
                </div>
              </fieldset>

              {f.role === "member" ? (
                <section
                  aria-label="App access"
                  className="flex flex-col gap-3 rounded-xl p-4 ring-1 ring-kumo-hairline"
                >
                  <Switch
                    label="All apps, including ones installed later"
                    checked={f.allApps}
                    onCheckedChange={(v) => setF({ ...f, allApps: v })}
                  />
                  {!f.allApps ? (
                    apps.length ? (
                      <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                        {apps.map((a) => (
                          <li
                            key={a.id}
                            className="flex items-center gap-2.5 rounded-lg px-2 py-1.5 hover:bg-kumo-tint"
                          >
                            <AppLogo
                              src={a.icon}
                              name={a.name}
                              category={a.category}
                              size="sm"
                            />
                            <Checkbox
                              label={a.name}
                              checked={f.apps.has(a.id)}
                              onCheckedChange={(v) =>
                                toggleApp(a.id, v === true)
                              }
                            />
                          </li>
                        ))}
                      </ul>
                    ) : (
                      <p className="text-xs text-kumo-subtle">
                        No apps installed yet. Install some, then choose here.
                      </p>
                    )
                  ) : null}
                </section>
              ) : null}

              {!isNew ? (
                <section
                  aria-label="Security"
                  className="flex flex-col divide-y divide-kumo-hairline rounded-xl ring-1 ring-kumo-hairline"
                >
                  <div className="flex flex-wrap items-center justify-between gap-3 p-4">
                    <span className="min-w-0">
                      <span className="block text-sm font-medium">
                        Two-factor
                      </span>
                      <span className="block text-xs text-kumo-subtle">
                        {user.totp
                          ? "An authenticator app is set up."
                          : "Not set up yet."}
                      </span>
                    </span>
                    <span className="flex gap-2">
                      {user.totp ? (
                        <>
                          <Button
                            size="sm"
                            variant="secondary"
                            onClick={() => setEnroll(true)}
                          >
                            Reset
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            loading={totpRemove.isPending}
                            onClick={() => totpRemove.mutate(user.username)}
                          >
                            Remove
                          </Button>
                        </>
                      ) : (
                        <Button
                          size="sm"
                          variant="secondary"
                          icon={<DeviceMobileIcon />}
                          onClick={() => setEnroll(true)}
                        >
                          Set Up 2FA
                        </Button>
                      )}
                    </span>
                  </div>
                  <div className="flex items-center justify-between gap-3 p-4">
                    <span className="min-w-0">
                      <span className="block text-sm font-medium">
                        Disable sign-in
                      </span>
                      <span className="block text-xs text-kumo-subtle">
                        Blocks this login without deleting it.
                      </span>
                    </span>
                    <Switch
                      checked={f.disabled}
                      onCheckedChange={(v) => setF({ ...f, disabled: v })}
                      aria-label="Disable sign-in"
                    />
                  </div>
                  <div className="flex flex-wrap items-center justify-between gap-3 p-4">
                    <span className="min-w-0">
                      <span className="block text-sm font-medium text-kumo-danger">
                        Delete login
                      </span>
                      <span className="block text-xs text-kumo-subtle">
                        {confirmDelete
                          ? "This can’t be undone. Their app data stays."
                          : "Removes the login and its two-factor."}
                      </span>
                    </span>
                    <Button
                      size="sm"
                      variant={confirmDelete ? "destructive" : "secondary"}
                      icon={confirmDelete ? <WarningIcon /> : undefined}
                      loading={remove.isPending}
                      onClick={() =>
                        confirmDelete
                          ? remove.mutate(user.username, { onSuccess: onClose })
                          : setConfirmDelete(true)
                      }
                    >
                      {confirmDelete ? "Delete for Good" : "Delete"}
                    </Button>
                  </div>
                </section>
              ) : null}

              {error ? (
                <Banner
                  variant="error"
                  title="Couldn’t save"
                  description={error.message}
                />
              ) : null}
            </div>
          </DialogFrame>
        </form>
        {enroll && user ? (
          <TwoFactorDialog
            user={user.username}
            onClose={() => setEnroll(false)}
          />
        ) : null}
      </Dialog>
    </Dialog.Root>
  );
}
