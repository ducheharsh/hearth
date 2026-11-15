//! Signed-in devices API.

use std::collections::{BTreeMap, BTreeSet};

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::error::ApiError;
use crate::sessions::{self, COOKIE};
use crate::state::SharedState;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/sessions", get(list))
        .route("/sessions/sign-out", post(sign_out))
        .route("/sessions/{id}", delete(remove))
}

fn err(e: anyhow::Error) -> ApiError {
    ApiError::Internal(e)
}

/// The session the request itself came with (behind the gateway, the browser's cookie).
fn current(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| c.trim().strip_prefix(COOKIE)?.strip_prefix('='))
        .map(sessions::hash)
}

/// Everyone signed in right now, newest activity first. A session shows up once it has
/// opened a site behind the login.
async fn list(
    State(st): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !st.settings.read().await.gateway_enabled {
        return Ok(Json(json!({ "running": false, "sessions": [] })));
    }
    let live = sessions::active(&st).await.map_err(err)?;
    let ids: BTreeSet<String> = live.into_keys().collect();
    let seen = {
        let mut s = st.sessions.write().await;
        s.retain(&ids);
        s.seen.clone()
    };
    let users: BTreeMap<String, String> = crate::gateway::load_users(&st)
        .unwrap_or_default()
        .into_iter()
        .map(|u| (u.username, u.display_name))
        .collect();
    let (installed, _) = st.installed().await;
    let names: BTreeMap<u16, String> = hearth_gateway::routes(st.app_routes(&installed).await)
        .into_iter()
        .filter(|r| r.port != hearth_gateway::AUTH_PORT)
        .map(|r| (r.port, r.name))
        .collect();
    let peers = hearth_host::tailscale::peers().await.unwrap_or_default();
    let me = current(&headers);
    // Only signed-in sessions: Authelia also keeps anonymous ones (the login page).
    let mut out: Vec<serde_json::Value> = ids
        .iter()
        .filter_map(|id| {
            seen.get(id)
                .filter(|s| s.user.is_some())
                .map(|s| (id, s.clone()))
        })
        .map(|(id, s)| {
            let node = s.ip.as_ref().and_then(|ip| {
                peers
                    .iter()
                    .find(|p| p.ips.contains(ip))
                    .map(|p| json!({ "name": p.name, "os": p.os, "ip": ip }))
            });
            json!({
                "id": id,
                "user": s.user,
                "display_name": s.user.as_ref().and_then(|u| users.get(u)),
                "device": sessions::describe(&s.agent),
                "node": node,
                "first_seen": s.first,
                "last_seen": s.last,
                "apps": s.ports.iter().filter_map(|p| names.get(p)).collect::<Vec<_>>(),
                "current": me.as_deref() == Some(id.as_str()),
            })
        })
        .collect();
    out.sort_by_key(|v| std::cmp::Reverse(v["last_seen"].as_u64().unwrap_or(0)));
    Ok(Json(json!({ "running": true, "sessions": out })))
}

async fn remove(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let n = sessions::sign_out(&st, &BTreeSet::from([id]))
        .await
        .map_err(err)?;
    if n == 0 {
        return Err(ApiError::NotFound("Session".into()));
    }
    Ok(Json(json!({ "signed_out": n })))
}

#[derive(Deserialize)]
struct SignOut {
    /// Only this user's devices; everyone when absent.
    user: Option<String>,
}

async fn sign_out(
    State(st): State<SharedState>,
    Json(b): Json<SignOut>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let n = if let Some(u) = b.user {
        sessions::sign_out_user(&st, &u).await
    } else {
        let all = sessions::active(&st).await.map_err(err)?;
        sessions::sign_out(&st, &all.into_keys().collect()).await
    }
    .map_err(err)?;
    st.event(
        crate::events::Level::Info,
        "security",
        "Signed out",
        format!("{n} device(s) signed out."),
    )
    .await;
    Ok(Json(json!({ "signed_out": n })))
}
