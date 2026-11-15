//! `GET /api/search?q=…&kinds=app,discover&limit=20`

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::bad;
use crate::error::ApiError;
use crate::state::SharedState;

pub fn routes() -> Router<SharedState> {
    Router::new().route("/search", get(search))
}

#[derive(Deserialize)]
struct Params {
    #[serde(default)]
    q: String,
    #[serde(default)]
    kinds: String,
    #[serde(default = "twenty")]
    limit: usize,
}
const fn twenty() -> usize {
    20
}

async fn search(
    State(st): State<SharedState>,
    Query(p): Query<Params>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if st.search.read().await.is_none() {
        crate::search::rebuild(&st).await;
    }
    let ix = st.search.read().await;
    let Some(ix) = ix.as_ref() else {
        return Ok(Json(json!({ "hits": [] })));
    };
    let kinds: Vec<&str> = p
        .kinds
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .collect();
    let started = std::time::Instant::now();
    let hits = ix
        .search(&p.q, &kinds, p.limit.clamp(1, 100))
        .map_err(bad)?;
    Ok(Json(
        json!({ "hits": hits, "took_ms": started.elapsed().as_secs_f64() * 1000.0, "indexed": ix.len() }),
    ))
}
