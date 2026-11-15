//! Containers: list, inspect, start/stop/restart, logs (tail + live SSE), and
//! adopting containers you run yourself into the gateway.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures_util::{Stream, StreamExt};
use hearth_host::docker::{Action, ContainerDetail, ContainerInfo, DockerClient, LogLine};
use serde::Deserialize;
use serde_json::json;

use super::bad;
use crate::error::{ApiError, internal};
use crate::events::Level;
use crate::state::{ExternalRoute, SharedState};
use crate::{gateway, settings::valid_label};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/containers", get(list))
        .route("/containers/{id}", get(inspect))
        .route("/containers/{id}/logs", get(logs))
        .route("/containers/{id}/logs/stream", get(stream))
        .route("/containers/{id}/{action}", post(act))
        .route("/routes", get(list_routes).post(adopt))
        .route("/routes/{id}", delete(unadopt))
}

fn docker(st: &SharedState) -> Result<&DockerClient, ApiError> {
    st.docker
        .as_ref()
        .ok_or_else(|| bad("Docker isn't reachable. Start OrbStack, then restart hearth."))
}

/// Container ids/names only: no path tricks into the Docker API.
fn check_id(id: &str) -> Result<(), ApiError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(bad("Invalid container id."));
    }
    Ok(())
}

async fn list(State(st): State<SharedState>) -> Result<Json<Vec<ContainerInfo>>, ApiError> {
    Ok(Json(docker(&st)?.containers().await.map_err(internal)?))
}

async fn inspect(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<ContainerDetail>, ApiError> {
    check_id(&id)?;
    Ok(Json(
        docker(&st)?
            .inspect(&id)
            .await
            .map_err(|e| ApiError::NotFound(e.to_string()))?,
    ))
}

async fn act(
    State(st): State<SharedState>,
    Path((id, action)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    check_id(&id)?;
    let a: Action = serde_json::from_value(json!(action))
        .map_err(|_| ApiError::NotFound(format!("action `{action}`")))?;
    docker(&st)?.act(&id, a).await.map_err(internal)?;
    st.event(Level::Info, "containers", format!("Container {action}"), id)
        .await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct Tail {
    #[serde(default = "default_tail")]
    tail: usize,
}
const fn default_tail() -> usize {
    300
}

async fn logs(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<Tail>,
) -> Result<Json<Vec<LogLine>>, ApiError> {
    check_id(&id)?;
    let lines: Vec<LogLine> = docker(&st)?
        .logs(&id, q.tail.min(5000), false)
        .filter_map(|r| async move { r.ok() })
        .collect()
        .await;
    Ok(Json(lines))
}

/// Live logs as Server-Sent Events (one JSON `LogLine` per event).
async fn stream(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    Query(q): Query<Tail>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    check_id(&id)?;
    let s = docker(&st)?
        .logs(&id, q.tail.min(1000), true)
        .filter_map(|r| async move {
            let line = r.ok()?;
            Some(Ok(Event::default().json_data(line).unwrap_or_default()))
        });
    Ok(Sse::new(s).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn list_routes(State(st): State<SharedState>) -> Json<Vec<ExternalRoute>> {
    Json(st.persisted.read().await.routes.clone())
}

#[derive(Deserialize)]
struct Adopt {
    /// Container name.
    container: String,
    name: String,
    container_port: u16,
    port: u16,
    #[serde(default = "yes")]
    protect: bool,
    #[serde(default)]
    exempt_paths: Vec<String>,
    #[serde(default)]
    public: bool,
}
const fn yes() -> bool {
    true
}

/// Publish a container you run yourself: join it to the `hearth` network and add a route.
async fn adopt(
    State(st): State<SharedState>,
    Json(a): Json<Adopt>,
) -> Result<Json<ExternalRoute>, ApiError> {
    check_id(&a.container)?;
    let id = a
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let id = id.trim_matches('-').to_owned();
    if !valid_label(&id) {
        return Err(bad("Name must contain letters or digits."));
    }
    if st.catalog.read().await.get(&id).is_some()
        || st.persisted.read().await.routes.iter().any(|r| r.id == id)
    {
        return Err(bad(format!("`{id}` is already used.")));
    }
    st.port_free(a.port, &id).await.map_err(bad)?;
    if a.exempt_paths
        .iter()
        .any(|p| !p.starts_with('/') || p.contains(char::is_whitespace))
    {
        return Err(bad(
            "Exempt paths must start with '/' and contain no spaces.",
        ));
    }
    docker(&st)?
        .inspect(&a.container)
        .await
        .map_err(|_| bad("No such container."))?;
    hearth_host::compose::ensure_network()
        .await
        .map_err(internal)?;
    if let Err(e) = hearth_host::run(
        "docker",
        &[
            "network",
            "connect",
            hearth_host::compose::NETWORK,
            &a.container,
        ],
        &[],
    )
    .await
    {
        if !e.to_string().contains("already exists") {
            return Err(internal(e));
        }
    }
    let route = ExternalRoute {
        id,
        name: a.name.trim().to_owned(),
        container: a.container,
        container_port: a.container_port,
        port: a.port,
        protect: a.protect,
        exempt_paths: a.exempt_paths,
        public: a.public,
    };
    {
        let mut p = st.persisted.write().await;
        p.routes.push(route.clone());
        st.save_persisted(&p).map_err(internal)?;
    }
    gateway::reapply_quietly(&st).await;
    st.event(
        Level::Success,
        "apps",
        format!("{} published", route.name),
        format!("Port {} now opens your container.", route.port),
    )
    .await;
    Ok(Json(route))
}

async fn unadopt(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let removed = {
        let mut p = st.persisted.write().await;
        let before = p.routes.len();
        p.routes.retain(|r| r.id != id);
        st.save_persisted(&p).map_err(internal)?;
        before != p.routes.len()
    };
    if !removed {
        return Err(ApiError::NotFound(format!("route `{id}`")));
    }
    gateway::reapply_quietly(&st).await;
    Ok(Json(json!({ "removed": id })))
}
