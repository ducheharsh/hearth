//! Always-on API: stay awake, power settings, connectivity.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::bad;
use crate::error::{ApiError, internal};
use crate::events::Level;
use crate::state::SharedState;
use crate::store;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/always-on", get(status).put(update))
        .route("/always-on/power", post(apply_power))
        .route("/always-on/wifi", post(wifi))
        .route("/always-on/check", post(check_now))
}

async fn status(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let (power, network) =
        tokio::join!(hearth_host::power::status(), hearth_host::network::status());
    let settings = st.settings.read().await.always_on;
    let mut a = st.always_on.lock().await;
    let holding = a.holding();
    Json(json!({
        "settings": settings,
        "holding": holding,
        "power": power.ok(),
        "network": network.ok(),
        "connectivity": a.conn,
    }))
}

#[derive(Deserialize)]
struct Update {
    keep_awake: Option<bool>,
    watch_connectivity: Option<bool>,
    auto_repair: Option<bool>,
}

async fn update(
    State(st): State<SharedState>,
    Json(u): Json<Update>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut s = st.settings.write().await;
    let a = &mut s.always_on;
    if let Some(v) = u.keep_awake {
        a.keep_awake = v;
    }
    if let Some(v) = u.watch_connectivity {
        a.watch_connectivity = v;
    }
    if let Some(v) = u.auto_repair {
        a.auto_repair = v;
    }
    let out = *a;
    store::save(&st.config.state_dir.join("settings.json"), &*s).map_err(internal)?;
    Ok(Json(json!(out)))
}

#[derive(Deserialize)]
struct Power {
    /// Laptops: also stay awake with the lid closed.
    #[serde(default)]
    lid_closed: bool,
}

/// Apply the server power settings. macOS asks the person at the Mac for an
/// administrator password in its own dialog.
async fn apply_power(
    State(st): State<SharedState>,
    Json(p): Json<Power>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let now = hearth_host::power::status().await.map_err(internal)?;
    let args = hearth_host::power::recommended_args(&now, p.lid_closed);
    if let Err(e) = hearth_host::power::apply_as_admin(&args).await {
        let msg = e.to_string();
        return Err(bad(if msg.contains("-128") || msg.contains("canceled") {
            "Cancelled on the Mac. Nothing was changed.".to_owned()
        } else if msg.contains("timed out") {
            "Nobody answered the password prompt on the Mac within 2 minutes.".to_owned()
        } else {
            format!("Couldn't change power settings: {msg}")
        }));
    }
    st.event(
        Level::Success,
        "power",
        "Power settings updated",
        "This Mac won't sleep on power, wakes for network access, and restarts after a power failure where supported.",
    )
    .await;
    Ok(Json(
        json!({ "power": hearth_host::power::status().await.ok() }),
    ))
}

#[derive(Deserialize)]
struct Wifi {
    on: bool,
}

async fn wifi(Json(w): Json<Wifi>) -> Result<Json<serde_json::Value>, ApiError> {
    let net = hearth_host::network::status().await.map_err(internal)?;
    let Some(dev) = net.wifi_device else {
        return Err(bad("This Mac has no Wi-Fi."));
    };
    hearth_host::network::set_wifi(&dev, w.on)
        .await
        .map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

async fn check_now(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let repair = st.settings.read().await.always_on.auto_repair;
    crate::always_on::check(&st, repair).await;
    Json(json!({ "connectivity": st.always_on.lock().await.conn }))
}
