//! Alerts API.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::bad;
use crate::alerts::{self, AlertSettings};
use crate::error::{ApiError, internal};
use crate::events::{Level, now};
use crate::state::SharedState;
use crate::store;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/alerts", get(get_alerts).put(put_alerts))
        .route("/alerts/test", post(test))
}

async fn get_alerts(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let secrets = alerts::load_secrets(&st);
    Json(json!({
        "settings": *st.alerts.read().await,
        "has_ntfy_token": secrets.ntfy_token.is_some(),
        "has_telegram_token": secrets.telegram_bot_token.is_some(),
    }))
}

#[derive(Deserialize)]
struct Body {
    settings: AlertSettings,
    ntfy_token: Option<String>,
    telegram_bot_token: Option<String>,
}

async fn put_alerts(
    State(st): State<SharedState>,
    Json(b): Json<Body>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let s = &b.settings;
    if s.ntfy.enabled && (s.ntfy.topic.trim().is_empty() || !s.ntfy.server.starts_with("http")) {
        return Err(bad("ntfy needs a server URL and a topic."));
    }
    if s.webhook.enabled && !s.webhook.url.starts_with("http") {
        return Err(bad("The webhook URL must start with http(s)://"));
    }
    if s.telegram.enabled && s.telegram.chat_id.trim().is_empty() {
        return Err(bad("Telegram needs a chat ID."));
    }
    let mut secrets = alerts::load_secrets(&st);
    if let Some(t) = b.ntfy_token.filter(|t| !t.trim().is_empty()) {
        secrets.ntfy_token = Some(t.trim().to_owned());
    }
    if let Some(t) = b.telegram_bot_token.filter(|t| !t.trim().is_empty()) {
        secrets.telegram_bot_token = Some(t.trim().to_owned());
    }
    store::save(&st.config.secrets_dir().join("alerts.json"), &secrets).map_err(internal)?;
    store::save(&st.config.state_dir.join("alerts.json"), &b.settings).map_err(internal)?;
    *st.alerts.write().await = b.settings;
    Ok(Json(json!({ "ok": true })))
}

/// Send a test notification to every enabled channel; report per-channel failures.
async fn test(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let settings = st.alerts.read().await.clone();
    let name = st.settings.read().await.display_name.clone();
    let e = crate::events::Event {
        seq: 0,
        at: now(),
        level: Level::Warning,
        source: "alerts",
        title: "Test alert".into(),
        message: "If you can read this, alerts work. 🔥".into(),
    };
    let out = alerts::build(&settings, &alerts::load_secrets(&st), &name, &e);
    if out.is_empty() {
        return Err(bad("Turn on at least one channel first."));
    }
    let n = out.len();
    let errors = alerts::send(out).await;
    Ok(Json(json!({ "sent": n - errors.len(), "errors": errors })))
}
