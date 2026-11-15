//! HTTP API (`/api/*`). The dashboard is served by [`crate::web`] as the fallback.

mod alerts;
pub(crate) mod apps;
mod backups;
mod containers;
mod discover;
mod packs;
mod search;
mod settings;
mod storage;
mod system;
mod terminal;
mod updates;

use std::time::Duration;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::error::ApiError;
use crate::state::SharedState;

pub fn router(state: SharedState) -> Router {
    let api = Router::new()
        .merge(system::routes())
        .merge(alerts::routes())
        .merge(apps::routes())
        .merge(backups::routes())
        .merge(containers::routes())
        .merge(storage::routes())
        .merge(settings::routes())
        .merge(discover::routes())
        .merge(terminal::routes())
        .merge(search::routes())
        .merge(updates::routes())
        .merge(packs::routes())
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .nest("/api", api)
        .fallback(crate::web::serve)
        .layer(TraceLayer::new_for_http())
        // Installs pull images and Cloudflare setup does several API calls: be generous.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(900),
        ))
        .with_state(state)
}

/// With `require_remote_user`, only requests that passed Authelia are served
/// (Caddy copies `Remote-User` from the auth response).
async fn require_auth(
    State(st): State<SharedState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let open = req.uri().path() == "/health";
    if st.settings.read().await.require_remote_user
        && !open
        && !req.headers().contains_key("remote-user")
    {
        return Err(ApiError::Unauthorized);
    }
    Ok(next.run(req).await)
}

/// `anyhow` → 400 with the message (user-facing validation and precondition errors).
pub(crate) fn bad(e: impl std::fmt::Display) -> ApiError {
    ApiError::BadRequest(e.to_string())
}
