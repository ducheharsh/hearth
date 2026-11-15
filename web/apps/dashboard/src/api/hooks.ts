import { keepPreviousData, useMutation, useQuery, useQueryClient, type QueryKey } from "@tanstack/react-query";
import { api, del, post, put } from "./client";
import type {
  AppConfig,
  AppConfigResponse,
  AppOverride,
  CatalogApp,
  CloudflareStatus,
  ContainerDetail,
  ContainerInfo,
  DiscoverMeta,
  DiscoverPage,
  DryRunResponse,
  ExternalRoute,
  GatewayResponse,
  GatewayUser,
  HeadscaleStatus,
  NetbirdStatus,
  HearthEvent,
  Leaderboards,
  LogLine,
  Role,
  SetupCheck,
  Settings,
  StorageResponse,
  SystemResponse,
  TailscaleStatus,
} from "./types";

export const keys = {
  system: ["system"],
  containers: ["containers"],
  catalog: ["catalog"],
  tailscale: ["tailscale"],
  storage: ["storage"],
  settings: ["settings"],
  setup: ["setup"],
  gateway: ["gateway"],
  users: ["users"],
  cloudflare: ["cloudflare"],
  routes: ["routes"],
} as const satisfies Record<string, QueryKey>;

const live = { placeholderData: keepPreviousData } as const;

export const useSystem = () =>
  useQuery({ queryKey: keys.system, queryFn: () => api<SystemResponse>("/system"), refetchInterval: 5_000, ...live });
export const useContainers = () =>
  useQuery({ queryKey: keys.containers, queryFn: () => api<ContainerInfo[]>("/containers"), refetchInterval: 5_000, ...live });
export const useCatalog = () =>
  useQuery({
    queryKey: keys.catalog,
    queryFn: () => api<{ apps: CatalogApp[]; month: string }>("/catalog"),
    refetchInterval: 15_000,
    ...live,
  });
export const useTailscale = () =>
  useQuery({ queryKey: keys.tailscale, queryFn: () => api<TailscaleStatus>("/tailscale"), refetchInterval: 10_000 });
export const useStorage = () =>
  useQuery({ queryKey: keys.storage, queryFn: () => api<StorageResponse>("/storage"), refetchInterval: 4_000, ...live });
export const useSettings = () => useQuery({ queryKey: keys.settings, queryFn: () => api<Settings>("/settings") });
export const useSetup = () =>
  useQuery({ queryKey: keys.setup, queryFn: () => api<SetupCheck[]>("/setup"), refetchInterval: 10_000 });
export const useGateway = () => useQuery({ queryKey: keys.gateway, queryFn: () => api<GatewayResponse>("/gateway") });
export const useUsers = () => useQuery({ queryKey: keys.users, queryFn: () => api<GatewayUser[]>("/gateway/users") });
export const useCloudflare = () =>
  useQuery({ queryKey: keys.cloudflare, queryFn: () => api<CloudflareStatus>("/cloudflare"), refetchInterval: 15_000 });
export const useRoutes = () => useQuery({ queryKey: keys.routes, queryFn: () => api<ExternalRoute[]>("/routes") });

export const useContainer = (id: string | null) =>
  useQuery({
    queryKey: ["container", id],
    queryFn: () => api<ContainerDetail>(`/containers/${id}`),
    enabled: id != null,
    refetchInterval: 5_000,
  });
export const useLogs = (id: string | null) =>
  useQuery({ queryKey: ["logs", id], queryFn: () => api<LogLine[]>(`/containers/${id}/logs?tail=400`), enabled: id != null });

export const useAppConfig = (id: string, enabled: boolean) =>
  useQuery({ queryKey: ["app-config", id], queryFn: () => api<AppConfigResponse>(`/apps/${id}/config`), enabled });

export function useDryRun(id: string, config: AppConfig | null, enabled: boolean) {
  return useQuery({
    queryKey: ["dry-run", id, config],
    queryFn: () => post<DryRunResponse>(`/apps/${id}/install`, { dry_run: true, config }),
    enabled,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

export interface DiscoverQuery {
  q: string;
  tag: string;
  sort: "stars" | "updated" | "name" | "activity";
  docker_only: boolean;
  page: number;
}

export function useDiscover(q: DiscoverQuery) {
  const params = new URLSearchParams({
    q: q.q,
    sort: q.sort,
    docker_only: String(q.docker_only),
    page: String(q.page),
    per_page: "36",
  });
  if (q.tag) params.set("tag", q.tag);
  return useQuery({ queryKey: ["discover", q], queryFn: () => api<DiscoverPage>(`/discover?${params}`), ...live });
}
export const useDiscoverMeta = () =>
  useQuery({ queryKey: ["discover-meta"], queryFn: () => api<DiscoverMeta>("/discover/meta"), refetchInterval: 30_000 });
export const useLeaderboards = () =>
  useQuery({ queryKey: ["leaderboards"], queryFn: () => api<Leaderboards>("/discover/leaderboards"), staleTime: 60_000 });

export const useEvents = (after: number) =>
  useQuery({
    queryKey: ["events", after],
    queryFn: () => api<{ last: number; events: HearthEvent[] }>(`/events?after=${after}`),
    refetchInterval: 3_000,
  });

/** Mutation that refreshes the given queries afterwards. */
function useAction<TVars, TOut>(fn: (v: TVars) => Promise<TOut>, invalidate: QueryKey[], key: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationKey: [key],
    mutationFn: fn,
    onSettled: () => Promise.all(invalidate.map((k) => qc.invalidateQueries({ queryKey: k }))),
  });
}

const appKeys = [keys.catalog, keys.containers, keys.setup, keys.gateway];

export const useInstall = () =>
  useAction(
    ({ id, force, config }: { id: string; force?: boolean; config?: AppConfig | null }) =>
      post<{ installed: string; gateway: unknown }>(`/apps/${id}/install`, { force: force ?? false, config }),
    appKeys,
    "install",
  );
export const useRemove = () =>
  useAction((id: string) => post<{ removed: string; data_kept_in: string }>(`/apps/${id}/remove`), appKeys, "remove");
export const useAppAction = () =>
  useAction(({ id, action }: { id: string; action: "start" | "stop" | "restart" }) => post(`/apps/${id}/${action}`), appKeys, "app-action");
export const useOverride = () =>
  useAction(({ id, o }: { id: string; o: AppOverride }) => put(`/apps/${id}/override`, o), [...appKeys, keys.settings], "override");
export const useRate = () =>
  useAction(({ key, stars }: { key: string; stars: number | null }) => put(`/ratings/${key}`, { stars }), [keys.catalog, ["discover"]], "rate");
export const useCreateCustom = () => useAction((body: Record<string, unknown>) => post<{ id: string }>("/custom-apps", body), [keys.catalog], "custom");
export const useDeleteCustom = () => useAction((id: string) => del(`/custom-apps/${id}`), [keys.catalog], "custom-delete");

export const useContainerAction = () =>
  useAction(({ id, action }: { id: string; action: "start" | "stop" | "restart" }) => post(`/containers/${id}/${action}`), [keys.containers, ["container"]], "container-action");
export const useAdopt = () => useAction((body: Record<string, unknown>) => post<ExternalRoute>("/routes", body), [keys.routes, keys.gateway], "adopt");
export const useUnadopt = () => useAction((id: string) => del(`/routes/${id}`), [keys.routes, keys.gateway], "unadopt");

const storageKeys = [keys.storage, keys.setup, keys.catalog];
export const useAttach = () =>
  useAction((b: { mount: string; role: Role; label?: string; force?: boolean }) => post("/storage/attach", b), storageKeys, "attach");
export const useDetach = () => useAction((id: string) => post(`/storage/${id}/detach`), storageKeys, "detach");
export const useEject = () => useAction((id: string) => post(`/storage/${id}/eject`), storageKeys, "eject");

const settingsKeys = [keys.settings, keys.setup, keys.gateway, keys.catalog, keys.tailscale, keys.cloudflare];
export const useSaveSettings = () => useAction((s: Settings) => put<Settings>("/settings", s), settingsKeys, "settings");
export const useTailscaleAction = () =>
  useAction(({ path, body }: { path: string; body?: unknown }) => (body ? put(`/tailscale/${path}`, body) : post(`/tailscale/${path}`)), settingsKeys, "tailscale");
export const useGatewayAction = () => useAction((path: "enable" | "disable" | "apply") => post(`/gateway/${path}`), settingsKeys, "gateway");
export const useAddUser = () =>
  useAction((u: { username: string; display_name: string; email: string; password: string }) => post("/gateway/users", u), [keys.users, keys.setup, keys.gateway], "add-user");
export const useDeleteUser = () => useAction((name: string) => del(`/gateway/users/${name}`), [keys.users, keys.setup], "delete-user");
export const useCloudflareConnect = () =>
  useAction((b: { api_token: string; domain: string }) => post("/cloudflare/connect", b), settingsKeys, "cloudflare");
export const useCloudflareDisconnect = () => useAction((b: { delete: boolean }) => post("/cloudflare/disconnect", b), settingsKeys, "cloudflare-off");
export const useFunnel = () => useAction((app: string | null) => put("/tunnels/funnel", { app }), settingsKeys, "funnel");
export const useNgrok = () =>
  useAction((b: { app: string | null; domain?: string; authtoken?: string }) => put("/tunnels/ngrok", b), settingsKeys, "ngrok");
export const useDiscoverSync = () => useAction(() => post("/discover/sync"), [["discover"], ["discover-meta"], ["leaderboards"], keys.catalog], "discover-sync");

export const useNetbird = () =>
  useQuery({ queryKey: ["netbird"], queryFn: () => api<NetbirdStatus>("/netbird"), refetchInterval: 10_000 });
export const useHeadscale = () =>
  useQuery({ queryKey: ["headscale"], queryFn: () => api<HeadscaleStatus>("/headscale"), refetchInterval: 15_000 });

const netKeys = [["netbird"], ["headscale"], keys.tailscale, keys.settings, keys.gateway, keys.setup];
export const useNetbirdAction = () =>
  useAction(({ path, body }: { path: "up" | "down" | "expose"; body?: unknown }) => (path === "expose" ? put(`/netbird/${path}`, body) : post(`/netbird/${path}`, body)), netKeys, "netbird");
export const useHeadscaleInstall = () => useAction((c: { hostname: string; acme_email: string; base_domain: string }) => post("/headscale/install", c), netKeys, "hs-install");
export const useHeadscaleUninstall = () => useAction(() => del("/headscale"), netKeys, "hs-uninstall");
export const useHeadscaleUser = () => useAction((name: string) => post("/headscale/users", { name }), netKeys, "hs-user");
export const useHeadscaleKey = () =>
  useAction((b: { user: number; reusable: boolean; hours: number }) => post<{ key?: string }>("/headscale/keys", b), netKeys, "hs-key");
export const useHeadscaleJoinSelf = () => useAction((auth_key: string) => post("/headscale/join-self", { auth_key }), netKeys, "hs-join");
export const useLoginServer = () =>
  useAction((b: { login_server: string; auth_key: string }) => post("/tailscale/login-server", b), netKeys, "login-server");

export interface AppHistory {
  summary: { points: [number, number, number][]; avg_mem: number; p95_mem: number; peak_mem: number; avg_cpu: number; hours: number } | null;
  estimate_mb: number;
  measured_mb: number | null;
}
export const useAppHistory = (id: string, enabled: boolean) =>
  useQuery({ queryKey: ["app-history", id], queryFn: () => api<AppHistory>(`/apps/${id}/history`), enabled, refetchInterval: 60_000 });

export interface BackupResult {
  at: number;
  ok: boolean;
  snapshot: string | null;
  bytes_added: number | null;
  files: number | null;
  error: string | null;
  seconds: number;
}
export type BackupDestination =
  | { kind: "local" }
  | { kind: "s3"; endpoint: string; bucket: string; path: string }
  | { kind: "b2"; bucket: string; path: string };
export interface BackupSettings {
  enabled: boolean;
  destination: BackupDestination;
  hour: number;
  retention: { daily: number; weekly: number; monthly: number };
  stop_apps: boolean;
  include_media: boolean;
}
export interface BackupsOverview {
  settings: BackupSettings;
  ready: boolean;
  has_credentials: boolean;
  last_nightly: string | null;
  apps: { id: string; name: string; icon_file: string; last: BackupResult | null; running: boolean }[];
  image: string;
}
export interface Snapshot {
  id: string;
  short_id: string;
  time: string;
  tags: string[];
  paths: string[];
  bytes: number | null;
}
export const useBackups = () =>
  useQuery({ queryKey: ["backups"], queryFn: () => api<BackupsOverview>("/backups"), refetchInterval: 5_000 });
export const useSnapshots = (id: string | null) =>
  useQuery({ queryKey: ["snapshots", id], queryFn: () => api<Snapshot[]>(`/backups/${id}/snapshots`), enabled: id != null, retry: false });
export const useBackupSettings = () =>
  useAction((b: { settings: BackupSettings; access_key_id?: string; secret_access_key?: string }) => put("/backups/settings", b), [["backups"]], "backup-settings");
export const useBackupInit = () => useAction(() => post<{ password: string }>("/backups/init"), [["backups"]], "backup-init");
export const useBackupRun = () => useAction((app: string | null) => post("/backups/run", { app }), [["backups"]], "backup-run");
export const useRestore = () =>
  useAction(({ id, snapshot }: { id: string; snapshot: string }) => post(`/backups/${id}/restore`, { snapshot }), [["backups"], ["snapshots"], keys.catalog], "restore");

export interface TotpEnrollment {
  uri: string;
  secret: string;
  qr_svg: string;
}
export const useTotpStart = () => useAction((user: string) => post<TotpEnrollment>(`/gateway/users/${user}/totp`), [keys.users], "totp-start");
export const useTotpVerify = () =>
  useAction(({ user, secret, code }: { user: string; secret: string; code: string }) => post(`/gateway/users/${user}/totp/verify`, { secret, code }), [keys.users], "totp-verify");
export const useTotpRemove = () => useAction((user: string) => del(`/gateway/users/${user}/totp`), [keys.users], "totp-remove");

export interface AppUpdateInfo {
  checked_at: number;
  available: boolean;
  services: { service: string; image: string; update: boolean }[];
  error: string | null;
  pinned: boolean;
}
export interface UpdatesOverview {
  apps: Record<string, AppUpdateInfo>;
  auto: string[];
  history: { app: string; at: number; ok: boolean; rolled_back: boolean; message: string }[];
  running: string[];
}
export const useUpdates = () => useQuery({ queryKey: ["updates"], queryFn: () => api<UpdatesOverview>("/updates"), refetchInterval: 10_000 });
const upKeys = [["updates"], keys.catalog, keys.containers];
export const useUpdateApp = () => useAction((id: string) => post(`/updates/${id}`), upKeys, "update-app");
export const useCheckUpdates = () => useAction(() => post("/updates/check"), upKeys, "check-updates");
export const useAutoUpdate = () => useAction(({ id, enabled }: { id: string; enabled: boolean }) => put(`/updates/${id}/auto`, { enabled }), upKeys, "auto-update");
export const useUnpin = () => useAction((id: string) => post(`/updates/${id}/unpin`), upKeys, "unpin");

export type MemberState = "installed" | "install" | "skip";
export interface PackInfo {
  pack: {
    id: string;
    name: string;
    tagline: string;
    description: string;
    icon: string;
    apps: { id: string; role: string; optional: boolean }[];
    next_steps: string[];
  };
  plan: {
    apps: { id: string; name: string; role: string; optional: boolean; ram_mb: number; state: MemberState }[];
    models: { name: string; role: string; optional: boolean; size_mb: number; ram_mb: number; state: MemberState }[];
    needs_ollama: boolean;
    added: { ram_mb: number; vault_gb: number; needs_vault: boolean };
    download_mb: number;
    complete: boolean;
  };
  verdict: import("./types").Verdict | null;
  ollama: { installed: boolean; running: boolean; version: string | null; models: string[] };
  job: {
    started: number;
    finished: number | null;
    steps: { label: string; state: "pending" | "running" | "done" | "failed"; progress: number | null; error: string | null }[];
  } | null;
}
export const usePacks = () =>
  useQuery({
    queryKey: ["packs"],
    queryFn: () => api<PackInfo[]>("/packs"),
    refetchInterval: (q) => (q.state.data?.some((p) => p.job && !p.job.finished) ? 1500 : 30_000),
  });
export const usePackPlan = (id: string, withIds: string[]) =>
  useQuery({
    queryKey: ["packs", id, "plan", withIds],
    queryFn: () => post<PackInfo>(`/packs/${id}/plan`, { with: withIds }),
    placeholderData: (prev) => prev,
  });
export const useInstallPack = () =>
  useAction(({ id, with: w, force }: { id: string; with: string[]; force?: boolean }) => post(`/packs/${id}/install`, { with: w, force }), [["packs"], keys.catalog], "install-pack");
