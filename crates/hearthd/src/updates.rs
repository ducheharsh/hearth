//! App updates with automatic rollback.
//!
//! Check: compare each service's registry digest with the local image (no download).
//! Update: pre-update backup → remember current digests → pull + recreate → health gate.
//! If the app isn't healthy within the window: pin the previous digests
//! (`hearth.pin.yml`) and restore the pre-update data snapshot.

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::{Context as _, bail};
use hearth_core::AppManifest;
use hearth_core::compose_config;
use hearth_host::compose::{PIN_FILE, render_pins};
use hearth_host::docker::has_digest;
use serde::{Deserialize, Serialize};

use crate::events::{Level, now};
use crate::state::SharedState;
use crate::{backup, store};

const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
const HEALTH_WINDOW: Duration = Duration::from_secs(120);
const STABLE_FOR: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceUpdate {
    pub service: String,
    pub image: String,
    pub update: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppUpdates {
    pub checked_at: u64,
    pub available: bool,
    pub services: Vec<ServiceUpdate>,
    pub error: Option<String>,
    /// Pinned to old images after a failed update.
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub app: String,
    pub at: u64,
    pub ok: bool,
    pub rolled_back: bool,
    pub message: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateState {
    pub apps: BTreeMap<String, AppUpdates>,
    /// App ids that update themselves after each check.
    pub auto: Vec<String>,
    pub history: Vec<HistoryEntry>,
    #[serde(skip)]
    pub running: Vec<String>,
}

pub fn load(state_dir: &std::path::Path) -> UpdateState {
    store::load(&state_dir.join("updates.json")).unwrap_or_default()
}

pub async fn save(st: &SharedState) -> anyhow::Result<()> {
    let u = st.updates.read().await;
    store::save(&st.config.state_dir.join("updates.json"), &*u)
}

/// `(service, image)` pairs actually deployed for the app.
fn deployed_images(st: &SharedState, app: &AppManifest) -> Vec<(String, String)> {
    let compose = std::fs::read_to_string(st.compose.app_dir(&app.id).join("compose.yml"))
        .unwrap_or_else(|_| app.compose.clone());
    compose_config::extract(&compose)
        .map(|svcs| {
            svcs.into_iter()
                .map(|s| (s.name, format!("{}:{}", s.image, s.tag)))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn check_app(st: &SharedState, app: &AppManifest) -> AppUpdates {
    let Some(docker) = &st.docker else {
        return AppUpdates {
            error: Some("Docker unavailable".into()),
            ..AppUpdates::default()
        };
    };
    let pinned = st.compose.app_dir(&app.id).join(PIN_FILE).exists();
    let mut services = Vec::new();
    let mut error = None;
    for (service, image) in deployed_images(st, app) {
        let local = docker.local_digests(&image).await.unwrap_or_default();
        match docker.remote_digest(&image).await {
            Ok(Some(remote)) => services.push(ServiceUpdate {
                update: !has_digest(&local, &remote),
                service,
                image,
            }),
            Ok(None) => services.push(ServiceUpdate {
                update: false,
                service,
                image,
            }),
            Err(e) => {
                error = Some(format!("{image}: {e}"));
                services.push(ServiceUpdate {
                    update: false,
                    service,
                    image,
                });
            }
        }
    }
    AppUpdates {
        checked_at: now(),
        available: services.iter().any(|s| s.update),
        services,
        error,
        pinned,
    }
}

pub async fn check_all(st: &SharedState) {
    let (installed, _) = st.installed().await;
    let apps: Vec<AppManifest> = {
        let c = st.catalog.read().await;
        installed
            .iter()
            .filter_map(|id| c.get(id))
            .cloned()
            .collect()
    };
    let mut newly = Vec::new();
    for app in &apps {
        let r = check_app(st, app).await;
        let before = st
            .updates
            .read()
            .await
            .apps
            .get(&app.id)
            .is_some_and(|a| a.available);
        if r.available && !before {
            newly.push(app.name.clone());
        }
        st.updates.write().await.apps.insert(app.id.clone(), r);
    }
    let _ = save(st).await;
    if !newly.is_empty() {
        st.event(
            Level::Info,
            "updates",
            "Updates available",
            newly.join(", "),
        )
        .await;
    }
}

/// Wait until every container of the app is running (and healthy if it has a
/// health check) continuously for `STABLE_FOR`, within `HEALTH_WINDOW`.
async fn healthy(st: &SharedState, app: &AppManifest) -> anyhow::Result<bool> {
    let docker = st.docker.as_ref().context("Docker unavailable")?;
    let project = app.project_name();
    let started = std::time::Instant::now();
    let mut stable_since: Option<std::time::Instant> = None;
    while started.elapsed() < HEALTH_WINDOW {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let list = docker.containers().await?;
        let ours: Vec<_> = list
            .iter()
            .filter(|c| c.project.as_deref() == Some(project.as_str()))
            .collect();
        let mut ok = !ours.is_empty();
        for c in &ours {
            let d = docker.inspect(&c.id).await?;
            let unhealthy = d.health.as_deref() == Some("unhealthy");
            let starting = d.health.as_deref() == Some("starting");
            if !d.running
                || unhealthy
                || starting
                || d.restart_count.unwrap_or(0) > 0 && d.status == "restarting"
            {
                ok = false;
            }
            if unhealthy {
                return Ok(false);
            }
        }
        if ok {
            let since = *stable_since.get_or_insert_with(std::time::Instant::now);
            if since.elapsed() >= STABLE_FOR {
                return Ok(true);
            }
        } else {
            stable_since = None;
        }
    }
    Ok(false)
}

/// Update one app; rolls back automatically if it doesn't come up healthy.
pub async fn update_app(st: &SharedState, app: &AppManifest) -> anyhow::Result<HistoryEntry> {
    let docker = st.docker.as_ref().context("Docker unavailable")?;
    st.updates.write().await.running.push(app.id.clone());
    let result = async {
        // 1. Safety net: a pre-update snapshot (if backups are set up).
        let snapshot = if backup::load_secrets(st).repo_password.is_some() {
            let r = backup::backup_app(st, app, Some("pre-update")).await.context("pre-update backup")?;
            if !r.ok {
                bail!("Pre-update backup failed, so the update was not attempted: {}", r.error.unwrap_or_default());
            }
            r.snapshot
        } else {
            None
        };
        // 2. Remember exactly what's running now.
        let mut pins = Vec::new();
        for (service, image) in deployed_images(st, app) {
            if let Some(d) = docker.local_digests(&image).await.unwrap_or_default().into_iter().next() {
                pins.push((service, d));
            }
        }
        // 3. Pull + recreate (any earlier pin is cleared first).
        let pin_path = st.compose.app_dir(&app.id).join(PIN_FILE);
        let _ = tokio::fs::remove_file(&pin_path).await;
        let env = st.compose_env().await;
        let proj = st.compose.project(app);
        let app_env = st.compose.env_for(app, &env).await?;
        proj.pull(&app_env).await.context("downloading the new version")?;
        proj.up(&app_env).await.context("starting the new version")?;
        // 4. Health gate.
        if healthy(st, app).await? {
            return Ok(HistoryEntry { app: app.id.clone(), at: now(), ok: true, rolled_back: false, message: "Updated and healthy.".into() });
        }
        // 5. Roll back: old images + pre-update data.
        tokio::fs::write(&pin_path, render_pins(&pins)).await?;
        if let Some(snap) = &snapshot {
            backup::restore(st, app, snap).await.context("restoring pre-update data")?;
        }
        let proj = st.compose.project(app);
        proj.up(&app_env).await.context("starting the previous version")?;
        Ok(HistoryEntry {
            app: app.id.clone(),
            at: now(),
            ok: false,
            rolled_back: true,
            message: if snapshot.is_some() {
                "New version wasn't healthy: rolled back to the previous version and data.".into()
            } else {
                "New version wasn't healthy: rolled back to the previous version (no backup to restore data from).".into()
            },
        })
    }
    .await;
    st.updates.write().await.running.retain(|x| x != &app.id);
    let entry = result.unwrap_or_else(|e| HistoryEntry {
        app: app.id.clone(),
        at: now(),
        ok: false,
        rolled_back: false,
        message: format!("{e:#}"),
    });
    {
        let mut u = st.updates.write().await;
        u.history.push(entry.clone());
        let n = u.history.len();
        if n > 100 {
            u.history.drain(0..n - 100);
        }
    }
    let fresh = check_app(st, app).await;
    st.updates.write().await.apps.insert(app.id.clone(), fresh);
    save(st).await?;
    let level = if entry.ok {
        Level::Success
    } else {
        Level::Error
    };
    let title = if entry.ok {
        format!("{} updated", app.name)
    } else if entry.rolled_back {
        format!("{} rolled back", app.name)
    } else {
        format!("{} update failed", app.name)
    };
    st.event(level, "updates", title, entry.message.clone())
        .await;
    Ok(entry)
}

/// Periodic checks; apps on auto-update are updated right after.
pub async fn run_checker(st: SharedState) {
    tokio::time::sleep(Duration::from_secs(60)).await;
    loop {
        check_all(&st).await;
        let auto: Vec<AppManifest> = {
            let u = st.updates.read().await;
            let c = st.catalog.read().await;
            u.auto
                .iter()
                .filter(|id| u.apps.get(*id).is_some_and(|a| a.available && !a.pinned))
                .filter_map(|id| c.get(id))
                .cloned()
                .collect()
        };
        for app in auto {
            let _ops = st.ops.lock().await;
            let _ = update_app(&st, &app).await;
        }
        tokio::time::sleep(CHECK_EVERY).await;
    }
}
