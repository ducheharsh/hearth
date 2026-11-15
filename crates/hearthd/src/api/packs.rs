//! Packs API.

use std::collections::BTreeSet;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use hearth_core::capacity::{self, Fit};
use hearth_core::pack::Pack;
use serde::Deserialize;
use serde_json::json;

use crate::error::ApiError;
use crate::packs;
use crate::state::SharedState;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/packs", get(list))
        .route("/packs/{id}/plan", post(plan))
        .route("/packs/{id}/install", post(install))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Choice {
    /// Optional apps/models to include.
    with: BTreeSet<String>,
    force: bool,
}

fn find(st: &SharedState, id: &str) -> Result<Pack, ApiError> {
    st.packs
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("pack `{id}`")))
}

async fn describe(
    st: &SharedState,
    m: &packs::Machine,
    pack: &Pack,
    with: &BTreeSet<String>,
) -> serde_json::Value {
    let plan = m.plan(st, pack, with).await;
    let ollama = &m.ollama;
    let verdict = match st.snapshot().await {
        Some(snap) => Some(capacity::evaluate_resources(
            &plan.added,
            &snap,
            &st.settings.read().await.capacity.policy(),
        )),
        None => None,
    };
    let job = st.pack_jobs.read().await.0.get(&pack.id).cloned();
    json!({ "pack": pack, "plan": plan, "verdict": verdict, "ollama": ollama, "job": job })
}

async fn list(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let m = packs::Machine::read(&st, st.packs.iter().any(|p| p.ollama.is_some())).await;
    let mut out = Vec::new();
    for p in &st.packs {
        out.push(describe(&st, &m, p, &BTreeSet::new()).await);
    }
    Json(json!(out))
}

async fn plan(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    body: Option<Json<Choice>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pack = find(&st, &id)?;
    let c = body.map(|Json(b)| b).unwrap_or_default();
    let m = packs::Machine::read(&st, pack.ollama.is_some()).await;
    Ok(Json(describe(&st, &m, &pack, &c.with).await))
}

async fn install(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    body: Option<Json<Choice>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pack = find(&st, &id)?;
    let c = body.map(|Json(b)| b).unwrap_or_default();
    if st
        .pack_jobs
        .read()
        .await
        .0
        .get(&id)
        .is_some_and(packs::Job::running)
    {
        return Err(ApiError::Conflict(
            json!({ "error": "This pack is already being installed." }),
        ));
    }
    let m = packs::Machine::read(&st, pack.ollama.is_some()).await;
    let d = describe(&st, &m, &pack, &c.with).await;
    if d["verdict"]["fit"] == json!(Fit::Deny) && !c.force {
        return Err(ApiError::Conflict(
            json!({ "error": "does not fit", "verdict": d["verdict"] }),
        ));
    }
    packs::start(&st, pack, c.with).await;
    Ok(Json(json!({ "started": true })))
}
