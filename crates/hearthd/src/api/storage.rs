//! Storage: drives, attachments, attach/detach/eject.

use std::path::PathBuf;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use hearth_core::storage::{self, Role};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::bad;
use crate::error::ApiError;
use crate::state::SharedState;
use crate::storage as mgr;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/storage", get(overview))
        .route("/storage/refresh", post(refresh))
        .route("/storage/attach", post(attach))
        .route("/storage/{id}/detach", post(detach))
        .route("/storage/{id}/eject", post(eject))
}

#[derive(Serialize)]
struct DriveView {
    #[serde(flatten)]
    volume: storage::Volume,
    /// Attached under this role, if any.
    attached_as: Option<Role>,
    /// Suitability for each role (for the attach dialog).
    assessments: Vec<(Role, storage::Assessment)>,
}

#[derive(Serialize)]
struct AttachmentView {
    #[serde(flatten)]
    attachment: storage::Attachment,
    status: storage::Status,
    free: Option<u64>,
    total: Option<u64>,
    link: PathBuf,
    apps: Vec<String>,
}

async fn overview(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let (atts, drives, error) = {
        let s = st.storage.read().await;
        let atts: Vec<(storage::Attachment, storage::Status)> = s
            .attachments
            .iter()
            .map(|a| (a.clone(), s.status(a)))
            .collect();
        let drives: Vec<DriveView> = s
            .volumes
            .iter()
            .map(|v| DriveView {
                attached_as: atts
                    .iter()
                    .find(|(_, st)| st.mount() == Some(&v.mount))
                    .map(|(a, _)| a.role),
                assessments: Role::ALL
                    .iter()
                    .map(|r| (*r, storage::assess(v, *r)))
                    .collect(),
                volume: v.clone(),
            })
            .collect();
        (atts, drives, s.error.clone())
    };
    let mut views = Vec::new();
    for (a, status) in atts {
        let vol = drives
            .iter()
            .find(|d| status.mount() == Some(&d.volume.mount));
        views.push(AttachmentView {
            free: vol.map(|d| d.volume.free),
            total: vol.map(|d| d.volume.total),
            link: st.config.role_link(a.role),
            apps: mgr::dependents(&st, a.role).await,
            status,
            attachment: a,
        });
    }
    Json(json!({ "attachments": views, "drives": drives, "error": error }))
}

async fn refresh(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    mgr::refresh(&st).await.map_err(bad)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct Attach {
    mount: PathBuf,
    role: Role,
    label: Option<String>,
    #[serde(default)]
    force: bool,
}

async fn attach(
    State(st): State<SharedState>,
    Json(a): Json<Attach>,
) -> Result<Json<mgr::AttachOutcome>, ApiError> {
    let _ops = st.ops.lock().await;
    Ok(Json(
        mgr::attach(&st, &a.mount, a.role, a.label, a.force)
            .await
            .map_err(|e| bad(format!("{e:#}")))?,
    ))
}

async fn detach(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    mgr::detach(&st, &id)
        .await
        .map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(json!({ "ok": true })))
}

async fn eject(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    mgr::eject(&st, &id)
        .await
        .map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(json!({ "ok": true })))
}
