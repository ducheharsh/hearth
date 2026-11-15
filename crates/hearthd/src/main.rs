//! `hearthd`: the hearth daemon.
//!
//! Runs natively on the host (launchd on macOS) so it can see real system metrics,
//! drives, native apps and the `tailscale` CLI. Serves a JSON API and the dashboard on
//! loopback; the Caddy + Authelia gateway is the way in from other devices.

mod api;
mod backup;
mod cloudflare;
mod config;
mod discover;
mod error;
mod events;
mod gateway;
mod headscale;
mod history;
mod search;
mod settings;
mod state;
mod storage;
mod store;
mod tunnels;
mod twofa;
mod updates;
mod web;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use hearth_core::Catalog;
use hearth_host::compose::ComposeRunner;
use hearth_host::docker::DockerClient;
use tokio::sync::{Mutex, RwLock};
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::{AppState, Metrics, load_custom};

#[derive(Debug, Parser)]
#[command(version, about = "hearth: self-hosting control plane")]
struct Cli {
    /// Path to hearth.toml (defaults apply if the file is missing).
    #[arg(short, long, env = "HEARTH_CONFIG", default_value = "hearth.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hearthd=info,tower_http=warn".into()),
        )
        .init();

    let cli = Cli::parse();
    let config = Config::load(&cli.config)?;
    std::fs::create_dir_all(&config.state_dir)
        .with_context(|| format!("creating {}", config.state_dir.display()))?;

    let builtin: Vec<_> = Catalog::load_dir(&config.catalog_dir)
        .with_context(|| format!("loading catalog from {}", config.catalog_dir.display()))?
        .iter()
        .cloned()
        .collect();
    let custom = load_custom(&config.custom_dir()).unwrap_or_else(|e| {
        tracing::error!(error = %e, "ignoring broken custom apps");
        Vec::new()
    });
    let catalog = Catalog::from_manifests(builtin.iter().cloned().chain(custom))?;
    tracing::info!(apps = catalog.len(), "catalog loaded");

    let docker = match DockerClient::connect().await {
        Ok(d) => Some(d),
        Err(e) => {
            tracing::warn!(error = %e, "Docker unavailable; container features disabled");
            None
        }
    };

    let settings = store::load(&config.state_dir.join("settings.json"))?;
    let persisted = store::load(&config.state_dir.join("state.json"))?;
    let attachments = storage::load_attachments(&config.state_dir)?;
    let ts = hearth_host::tailscale::status().await.unwrap_or_default();

    let listen = config.listen;
    let state_dir_for_history = config.state_dir.clone();
    let state = Arc::new(AppState {
        compose: ComposeRunner::new(config.apps_dir()),
        config,
        builtin,
        catalog: RwLock::new(catalog),
        docker,
        settings: RwLock::new(settings),
        tailscale: RwLock::new(ts),
        metrics: RwLock::new(Metrics::default()),
        storage: RwLock::new(storage::StorageState {
            attachments,
            ..Default::default()
        }),
        discover: RwLock::new(discover::DiscoverState::default()),
        persisted: RwLock::new(persisted),
        events: Mutex::new(events::EventLog::default()),
        search: RwLock::new(None),
        history: RwLock::new(history::load(&state_dir_for_history)),
        backups: RwLock::new(backup::load(&state_dir_for_history)),
        updates: RwLock::new(updates::load(&state_dir_for_history)),
        ops: Mutex::new(()),
    });

    tokio::spawn(state::run_sampler(state.clone()));
    tokio::spawn(state::run_tailscale_poller(state.clone()));
    tokio::spawn(storage::run_watcher(state.clone()));
    tokio::spawn(discover::run_syncer(state.clone()));
    tokio::spawn(search::run_indexer(state.clone()));
    tokio::spawn(history::run_recorder(state.clone()));
    tokio::spawn(backup::run_scheduler(state.clone()));
    tokio::spawn(updates::run_checker(state.clone()));

    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .with_context(|| format!("binding {listen}"))?;
    tracing::info!("listening on http://{listen}");
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    tokio::select! { () = ctrl_c => {}, () = term => {} }
    tracing::info!("shutting down");
}
