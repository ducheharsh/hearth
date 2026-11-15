//! Backups API.

use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::bad;
use crate::backup::{self, BackupSettings, Destination};
use crate::error::{ApiError, internal};
use crate::state::SharedState;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/backups", get(overview))
        .route("/backups/settings", put(put_settings))
        .route("/backups/init", post(init))
        .route("/backups/password", get(password))
        .route("/backups/run", post(run_now))
        .route("/backups/{id}/snapshots", get(snapshots))
        .route("/backups/{id}/restore", post(restore))
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(err)`
fn err(e: anyhow::Error) -> ApiError {
    bad(format!("{e:#}"))
}

async fn overview(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let secrets = backup::load_secrets(&st);
    let (installed, _) = st.installed().await;
    let b = st.backups.read().await;
    let catalog = st.catalog.read().await;
    let apps: Vec<_> = installed
        .iter()
        .filter_map(|id| catalog.get(id))
        .map(|a| json!({ "id": a.id, "name": a.name, "icon_file": a.icon_file(), "last": b.results.get(&a.id), "running": b.running.contains(&a.id) }))
        .collect();
    Json(json!({
        "settings": b.settings,
        "ready": secrets.repo_password.is_some(),
        "has_credentials": secrets.access_key_id.is_some(),
        "last_nightly": b.last_nightly,
        "apps": apps,
        "image": backup::IMAGE,
    }))
}

#[derive(Deserialize)]
struct SettingsBody {
    settings: BackupSettings,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
}

async fn put_settings(
    State(st): State<SharedState>,
    Json(b): Json<SettingsBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if b.settings.hour > 23 {
        return Err(bad("Hour must be 0-23."));
    }
    let r = b.settings.retention;
    if r.daily + r.weekly + r.monthly == 0 {
        return Err(bad("Keep at least one snapshot."));
    }
    match &b.settings.destination {
        Destination::S3 {
            endpoint, bucket, ..
        } if endpoint.trim().is_empty() || bucket.trim().is_empty() => {
            return Err(bad("S3 needs an endpoint and a bucket."));
        }
        Destination::B2 { bucket, .. } if bucket.trim().is_empty() => {
            return Err(bad("B2 needs a bucket."));
        }
        _ => {}
    }
    let mut secrets = backup::load_secrets(&st);
    if let Some(k) = b.access_key_id.filter(|k| !k.trim().is_empty()) {
        secrets.access_key_id = Some(k.trim().to_owned());
    }
    if let Some(k) = b.secret_access_key.filter(|k| !k.trim().is_empty()) {
        secrets.secret_access_key = Some(k.trim().to_owned());
    }
    backup::save_secrets(&st, &secrets).map_err(internal)?;
    st.backups.write().await.settings = b.settings;
    backup::save_state(&st).await.map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

async fn init(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let password = backup::init(&st).await.map_err(err)?;
    Ok(Json(json!({ "password": password })))
}

async fn password(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let p = backup::load_secrets(&st)
        .repo_password
        .ok_or_else(|| bad("Backups aren't set up yet."))?;
    Ok(Json(json!({ "password": p })))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RunBody {
    app: Option<String>,
}

/// Start a backup in the background; progress arrives as events.
async fn run_now(
    State(st): State<SharedState>,
    body: Option<Json<RunBody>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app = body.and_then(|Json(b)| b.app);
    if backup::load_secrets(&st).repo_password.is_none() {
        return Err(bad("Set up backups first."));
    }
    let s = st.clone();
    match app {
        Some(id) => {
            let m = s
                .catalog
                .read()
                .await
                .get(&id)
                .cloned()
                .ok_or_else(|| ApiError::NotFound(format!("app `{id}`")))?;
            tokio::spawn(async move {
                let _ops = s.ops.lock().await;
                if backup::backup_app(&s, &m, Some("manual")).await.is_ok() {
                    let _ = backup::prune_app(&s, &m.id).await;
                }
            });
        }
        None => {
            tokio::spawn(async move {
                let _ = backup::run_all(&s).await;
            });
        }
    }
    Ok(Json(json!({ "started": true })))
}

async fn snapshots(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!(backup::snapshots(&st, &id).await.map_err(err)?)))
}

#[derive(Deserialize)]
struct RestoreBody {
    snapshot: String,
}

async fn restore(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    Json(r): Json<RestoreBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app = st
        .catalog
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("app `{id}`")))?;
    let _ops = st.ops.lock().await;
    backup::restore(&st, &app, r.snapshot.trim())
        .await
        .map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}
