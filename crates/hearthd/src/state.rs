//! Shared application state, plus helpers every handler needs.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use hearth_core::storage::Role;
use hearth_core::{AppManifest, Catalog, HostSnapshot};
use hearth_gateway::Route;
use hearth_host::compose::{ComposeEnv, ComposeRunner, path_str};
use hearth_host::docker::DockerClient;
use hearth_host::system::{SystemSample, SystemSampler};
use hearth_host::tailscale::TailscaleStatus;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::discover::DiscoverState;
use crate::events::{EventLog, Level};
use crate::settings::Settings;
use crate::storage::StorageState;
use crate::store;

/// 1 hour of history at a 5 s interval.
pub const HISTORY_LEN: usize = 720;
pub const SAMPLE_EVERY: Duration = Duration::from_secs(5);

pub type SharedState = Arc<AppState>;

/// A container the user brought themselves and published through the gateway.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalRoute {
    pub id: String,
    pub name: String,
    /// Container name on the `hearth` network.
    pub container: String,
    pub container_port: u16,
    pub port: u16,
    pub protect: bool,
    #[serde(default)]
    pub exempt_paths: Vec<String>,
    #[serde(default)]
    pub public: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Persisted {
    pub routes: Vec<ExternalRoute>,
    /// App id or discover slug → 1..=5 stars.
    pub ratings: BTreeMap<String, u8>,
}

#[derive(Debug)]
pub struct AppState {
    pub config: Config,
    /// Curated manifests shipped in `catalog/`.
    pub builtin: Vec<AppManifest>,
    /// Curated + custom apps.
    pub catalog: RwLock<Catalog>,
    pub docker: Option<DockerClient>,
    pub compose: ComposeRunner,
    pub settings: RwLock<Settings>,
    pub tailscale: RwLock<TailscaleStatus>,
    pub metrics: RwLock<Metrics>,
    pub storage: RwLock<StorageState>,
    pub discover: RwLock<DiscoverState>,
    pub persisted: RwLock<Persisted>,
    pub events: Mutex<EventLog>,
    pub search: RwLock<Option<hearth_search::SearchIndex>>,
    pub history: RwLock<crate::history::History>,
    pub backups: RwLock<crate::backup::BackupState>,
    pub updates: RwLock<crate::updates::UpdateState>,
    pub alerts: RwLock<crate::alerts::AlertSettings>,
    pub alert_tx: tokio::sync::mpsc::UnboundedSender<crate::events::Event>,
    /// Serialises installs, gateway applies and other mutations.
    pub ops: Mutex<()>,
}

#[derive(Debug, Default)]
pub struct Metrics {
    pub latest: Option<SystemSample>,
    pub history: VecDeque<Point>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Point {
    pub at: u64,
    pub cpu: f32,
    pub mem_used: u64,
    pub swap_used: u64,
}

impl Metrics {
    pub fn push(&mut self, s: SystemSample) {
        if self.history.len() == HISTORY_LEN {
            self.history.pop_front();
        }
        self.history.push_back(Point {
            at: s.at,
            cpu: s.cpu_percent,
            mem_used: s.mem_used,
            swap_used: s.swap_used,
        });
        self.latest = Some(s);
    }
}

impl AppState {
    pub async fn event(
        &self,
        level: Level,
        source: &'static str,
        title: impl Into<String>,
        msg: impl Into<String>,
    ) {
        let e = self.events.lock().await.push(level, source, title, msg);
        let _ = self.alert_tx.send(e);
    }

    pub async fn ts_host(&self) -> String {
        if let Some(h) = &self.config.ts_host {
            return h.clone();
        }
        self.tailscale
            .read()
            .await
            .dns_name
            .clone()
            .unwrap_or_else(|| "localhost".into())
    }

    /// Variables shared by every app's compose.
    pub async fn compose_env(&self) -> ComposeEnv {
        let tz = self.settings.read().await.tz.clone();
        let mut env = ComposeEnv::default()
            .with("TZ", tz)
            .with("TS_HOST", self.ts_host().await);
        for role in Role::ALL {
            env = env.with(role.env_var(), path_str(&self.config.role_link(role)));
        }
        env
    }

    pub fn save_persisted(&self, p: &Persisted) -> anyhow::Result<()> {
        store::save(&self.config.state_dir.join("state.json"), p)
    }

    /// Capacity inputs: latest metrics + vault free space.
    pub async fn snapshot(&self) -> Option<HostSnapshot> {
        let latest = self.metrics.read().await.latest.clone()?;
        let vault_free = self.storage.read().await.free_for(Role::Vault);
        Some(latest.snapshot(vault_free))
    }

    /// Catalog apps that exist as compose projects (`installed`) and have a running container.
    pub async fn installed(&self) -> (BTreeSet<String>, BTreeSet<String>) {
        let Some(docker) = &self.docker else {
            return Default::default();
        };
        let Ok(list) = docker.containers().await else {
            return Default::default();
        };
        let catalog = self.catalog.read().await;
        let mut installed = BTreeSet::new();
        let mut running = BTreeSet::new();
        for c in list {
            let Some(id) = c.project.as_deref().and_then(|p| p.strip_prefix("hearth-")) else {
                continue;
            };
            if catalog.get(id).is_some() {
                installed.insert(id.to_owned());
                if c.state == "running" {
                    running.insert(id.to_owned());
                }
            }
        }
        (installed, running)
    }

    /// All gateway routes for installed apps and adopted containers.
    pub async fn app_routes(&self, installed: &BTreeSet<String>) -> Vec<Route> {
        let catalog = self.catalog.read().await;
        let settings = self.settings.read().await;
        let mut routes: Vec<Route> = installed
            .iter()
            .filter_map(|id| catalog.get(id))
            .map(|app| {
                let o = settings.apps.get(&app.id).cloned().unwrap_or_default();
                let mut r = Route::for_app(app);
                r.port = o.port.unwrap_or(app.port);
                if let Some(s) = o.subdomain {
                    r.subdomain = s;
                }
                r.public = o.public;
                if app.id == "syncthing" {
                    r.host_header = Some("localhost:8384".into());
                }
                r
            })
            .collect();
        for e in &self.persisted.read().await.routes {
            routes.push(Route {
                id: e.id.clone(),
                name: e.name.clone(),
                port: e.port,
                upstream: format!("{}:{}", e.container, e.container_port),
                protect: e.protect,
                exempt_paths: e.exempt_paths.clone(),
                subdomain: e.id.clone(),
                public: e.public,
                host_header: None,
            });
        }
        routes
    }

    /// Is `port` free for `owner` (not reserved, not used by another app or route)?
    pub async fn port_free(&self, port: u16, owner: &str) -> Result<(), String> {
        if port < 1024
            || hearth_core::manifest::RESERVED_PORTS.contains(&port)
            || port == hearth_gateway::HEARTH_PORT
        {
            return Err(format!("Port {port} is reserved. Use 1024-65535."));
        }
        let catalog = self.catalog.read().await;
        let settings = self.settings.read().await;
        for app in catalog.iter().filter(|a| a.id != owner) {
            let p = settings
                .apps
                .get(&app.id)
                .and_then(|o| o.port)
                .unwrap_or(app.port);
            if p == port {
                return Err(format!("Port {port} is already used by {}.", app.name));
            }
        }
        if let Some(r) = self
            .persisted
            .read()
            .await
            .routes
            .iter()
            .find(|r| r.id != owner && r.port == port)
        {
            return Err(format!("Port {port} is already used by {}.", r.name));
        }
        Ok(())
    }

    /// Rebuild the catalog from built-in + custom manifests.
    pub async fn reload_catalog(&self) -> anyhow::Result<()> {
        let custom = load_custom(&self.config.custom_dir())?;
        let all = self.builtin.iter().cloned().chain(custom);
        *self.catalog.write().await = Catalog::from_manifests(all)?;
        Ok(())
    }
}

/// Custom app manifests the user created (`state_dir/custom/*.yml`).
pub fn load_custom(dir: &std::path::Path) -> anyhow::Result<Vec<AppManifest>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    Ok(Catalog::load_dir(dir)?.iter().cloned().collect())
}

/// Sample the host forever. `sysinfo` is blocking, so each sample runs on the blocking pool.
pub async fn run_sampler(state: SharedState) {
    let mut sampler = match SystemSampler::new() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "host metrics unsupported; sampler disabled");
            return;
        }
    };
    let mut tick = tokio::time::interval(SAMPLE_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        match tokio::task::spawn_blocking(move || {
            let sample = sampler.sample();
            (sampler, sample)
        })
        .await
        {
            Ok((s, sample)) => {
                sampler = s;
                state.metrics.write().await.push(sample);
            }
            Err(e) => {
                tracing::error!(error = %e, "sampler panicked; stopping");
                return;
            }
        }
    }
}

/// Keep the cached Tailscale status fresh.
pub async fn run_tailscale_poller(state: SharedState) {
    let mut tick = tokio::time::interval(Duration::from_secs(15));
    loop {
        tick.tick().await;
        match hearth_host::tailscale::status().await {
            Ok(s) => *state.tailscale.write().await = s,
            Err(e) => tracing::debug!(error = %e, "tailscale status failed"),
        }
    }
}
