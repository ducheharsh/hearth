//! Updates API.

use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::error::{ApiError, internal};
use crate::state::SharedState;
use crate::updates;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/updates", get(overview))
        .route("/updates/check", post(check))
        .route("/updates/{id}", post(update))
        .route("/updates/{id}/auto", put(auto))
        .route("/updates/{id}/unpin", post(unpin))
}

async fn overview(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let u = st.updates.read().await;
    Json(
        json!({ "apps": u.apps, "auto": u.auto, "history": u.history.iter().rev().take(20).collect::<Vec<_>>(), "running": u.running }),
    )
}

async fn check(State(st): State<SharedState>) -> Json<serde_json::Value> {
    updates::check_all(&st).await;
    Json(json!({ "ok": true }))
}

/// Start an update in the background; the outcome arrives as an event.
async fn update(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app = st
        .catalog
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("app `{id}`")))?;
    let s = st.clone();
    tokio::spawn(async move {
        let _ops = s.ops.lock().await;
        let _ = updates::update_app(&s, &app).await;
    });
    Ok(Json(json!({ "started": true })))
}

#[derive(Deserialize)]
struct Auto {
    enabled: bool,
}

async fn auto(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    Json(a): Json<Auto>,
) -> Result<Json<serde_json::Value>, ApiError> {
    {
        let mut u = st.updates.write().await;
        u.auto.retain(|x| x != &id);
        if a.enabled {
            u.auto.push(id);
        }
    }
    updates::save(&st).await.map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

/// Drop the rollback pin so the next update can move forward again.
async fn unpin(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ =
        tokio::fs::remove_file(st.compose.app_dir(&id).join(hearth_host::compose::PIN_FILE)).await;
    if let Some(a) = st.updates.write().await.apps.get_mut(&id) {
        a.pinned = false;
    }
    updates::save(&st).await.map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}
