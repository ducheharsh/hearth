/** Mirrors the JSON returned by hearthd (crates/hearthd/src/api/*). */

export interface DiskUsage {
  mount: string;
  total: number;
  free: number;
}

export interface SystemSample {
  at: number;
  hostname: string;
  os: string;
  uptime_secs: number;
  cpu_count: number;
  cpu_percent: number;
  load_avg: [number, number, number];
  mem_total: number;
  mem_used: number;
  mem_available: number;
  swap_total: number;
  swap_used: number;
  disks: DiskUsage[];
}

export interface HistoryPoint {
  at: number;
  cpu: number;
  mem_used: number;
  swap_used: number;
}

export interface SystemResponse {
  latest: SystemSample | null;
  history: HistoryPoint[];
  display_name: string;
}

export interface ContainerInfo {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  project: string | null;
  mem_bytes: number | null;
  cpu_percent: number | null;
}

export interface ContainerDetail {
  id: string;
  name: string;
  image: string;
  created: string | null;
  status: string;
  running: boolean;
  started_at: string | null;
  health: string | null;
  exit_code: number | null;
  restart_count: number | null;
  restart_policy: string | null;
  project: string | null;
  service: string | null;
  ports: { container: string; host: string[] }[];
  mounts: { kind: string; source: string; destination: string; read_only: boolean }[];
  networks: string[];
  env: string[];
}

export interface LogLine {
  stream: "stdout" | "stderr";
  text: string;
}

export type Fit = "ok" | "warn" | "deny";

export interface Verdict {
  fit: Fit;
  reasons: string[];
  mem_after: number;
}

export interface AppStats {
  stars: number | null;
  updated_at: string | null;
  release: string | null;
  commits_this_month: number;
  licenses: string[];
}

export interface CatalogApp {
  id: string;
  name: string;
  description: string;
  category: string;
  website: string | null;
  repo: string | null;
  featured: boolean;
  editors_note: string | null;
  icon: string | null;
  port: number;
  url: string;
  domain_url: string | null;
  subdomain: string;
  public: boolean;
  ram_mb: number;
  needs_vault: boolean;
  roles: string[];
  protected: boolean;
  custom: boolean;
  installed: boolean;
  running: boolean;
  verdict: Verdict | null;
  stats: AppStats | null;
  rating: number | null;
}

export interface EnvVar {
  key: string;
  value: string;
  secret: boolean;
}

export interface ServiceConfig {
  name: string;
  image: string;
  tag: string;
  environment: EnvVar[];
  ports: string[];
  volumes: string[];
  restart: string;
  mem_limit: string | null;
  cpus: string | null;
  command: string | null;
  privileged: boolean;
  read_only: boolean;
}

export type AppConfig = { mode: "form"; services: ServiceConfig[] } | { mode: "raw"; compose: string };

export interface AppConfigResponse {
  defaults: ServiceConfig[];
  saved: AppConfig | null;
  compose: string;
  restart_policies: string[];
}

export interface InstallPlan {
  project: string;
  dir: string;
  files: string[];
  command: string;
}

export interface DryRunResponse {
  dry_run: true;
  verdict: Verdict;
  plan: InstallPlan;
  compose: string;
}

export interface TailscaleStatus {
  installed: boolean;
  version: string | null;
  backend_state: string;
  auth_url: string | null;
  dns_name: string | null;
  hostname: string | null;
  ips: string[];
  tailnet: string | null;
  magic_dns: boolean;
  https_enabled: boolean;
}

export interface HearthEvent {
  seq: number;
  at: number;
  level: "info" | "success" | "warning" | "error";
  source: string;
  title: string;
  message: string;
}

export interface SetupCheck {
  id: string;
  title: string;
  state: "done" | "todo" | "warn" | "optional";
  detail: string;
  tab: string;
}

export type Role = "vault" | "backups" | "media";

export interface Assessment {
  level: "ok" | "warn" | "deny";
  notes: string[];
}

export interface Marker {
  id: string;
  role: Role;
  label: string;
  host: string;
}

export interface Drive {
  mount: string;
  name: string;
  volume_uuid: string | null;
  filesystem: string;
  protocol: string | null;
  device: string;
  writable: boolean;
  ejectable: boolean;
  total: number;
  free: number;
  marker: Marker | null;
  attached_as: Role | null;
  assessments: [Role, Assessment][];
}

export type DriveStatus =
  | { state: "online"; mount: string; matched_by: "marker" | "volume_uuid" }
  | { state: "offline"; impostor_at: string | null };

export interface AttachmentView {
  id: string;
  role: Role;
  label: string;
  volume_uuid: string | null;
  attached_at: number;
  last_mount: string | null;
  last_seen: number | null;
  status: DriveStatus;
  free: number | null;
  total: number | null;
  link: string;
  apps: string[];
}

export interface StorageResponse {
  attachments: AttachmentView[];
  drives: Drive[];
  error: string | null;
}

export interface AppOverride {
  port: number | null;
  subdomain: string | null;
  public: boolean;
}

export interface Settings {
  display_name: string;
  tz: string;
  capacity: { mem_reserve_mb: number; mem_warn_headroom_mb: number; disk_reserve_gb: number };
  require_remote_user: boolean;
  gateway_enabled: boolean;
  domain: string | null;
  cloudflare: { account_id: string | null; zone_id: string | null; tunnel_id: string | null; enabled: boolean };
  discover_enabled: boolean;
  auto_pause_on_disconnect: boolean;
  apps: Record<string, AppOverride>;
  tunnels: { funnel_app: string | null; ngrok_app: string | null; ngrok_domain: string | null };
  netbird_enabled: boolean;
  /** Who issues tailnet HTTPS certificates: Tailscale, or hearth's own CA. */
  tailscale_certs: TsCerts;
}

export type TsCerts = "tailscale" | "hearth";

export interface GatewayRoute {
  id: string;
  name: string;
  port: number;
  upstream: string;
  protect: boolean;
  exempt_paths: string[];
  subdomain: string;
  public: boolean;
}

export interface GatewayJobStep {
  label: string;
  state: "pending" | "running" | "done" | "failed";
  detail: string | null;
}
export interface GatewayJob {
  started: number;
  finished: number | null;
  ok: boolean;
  steps: GatewayJobStep[];
}
export interface GatewayResponse {
  enabled: boolean;
  users: number;
  preview: { routes: GatewayRoute[]; caddyfile: string; compose: string; authelia: string };
  job: GatewayJob | null;
}

export type UserRole = "admin" | "member";
export interface GatewayUser {
  username: string;
  display_name: string;
  email: string;
  totp: boolean;
  role: UserRole;
  disabled: boolean;
  /** Members: null = every app. */
  apps: string[] | null;
}

export interface CloudflareStatus {
  domain: string | null;
  enabled: boolean;
  tunnel_id: string | null;
  has_token: boolean;
  connector_running: boolean;
  public_apps: string[];
}

export interface ExternalRoute {
  id: string;
  name: string;
  container: string;
  container_port: number;
  port: number;
  protect: boolean;
  exempt_paths: string[];
  public: boolean;
}

export interface DiscoverEntry {
  slug: string;
  name: string;
  description: string;
  website: string | null;
  source: string | null;
  demo: string | null;
  repo: string | null;
  licenses: string[];
  platforms: string[];
  tags: string[];
  stars: number | null;
  updated_at: string | null;
  archived: boolean;
  release: { tag: string; published_at: string | null } | null;
  commits: Record<string, number>;
  depends_3rdparty: boolean;
  has_icon: boolean;
}

export interface DiscoverPage {
  total: number;
  page: number;
  per_page: number;
  items: { entry: DiscoverEntry; rating: number | null }[];
}

export interface DiscoverMeta {
  total: number;
  tags: Record<string, number>;
  synced_at: number | null;
  syncing: boolean;
  error: string | null;
  month: string | null;
  attribution: string;
  source: string;
}

export interface Leaderboards {
  month: string;
  top_month: DiscoverEntry[];
  most_starred: DiscoverEntry[];
  rising: DiscoverEntry[];
  fresh_releases: DiscoverEntry[];
}

export interface NetbirdStatus {
  installed: boolean;
  connected: boolean;
  version: string | null;
  ip: string | null;
  fqdn: string | null;
  management_url: string | null;
  peers_connected: number;
  peers_total: number;
}

export interface HeadscaleConfig {
  hostname: string;
  acme_email: string;
  base_domain: string;
}

export interface HeadscaleUser {
  id: number | string;
  name: string;
}

export interface HeadscaleNode {
  id: number | string;
  name?: string;
  given_name?: string;
  ip_addresses?: string[];
  online?: boolean;
  user?: { name?: string };
  last_seen?: { seconds?: number } | string;
}

export interface HeadscaleStatus {
  config: HeadscaleConfig | null;
  running: boolean;
  reachable: boolean | null;
  users: HeadscaleUser[] | null;
  nodes: HeadscaleNode[] | null;
  image: string;
}
