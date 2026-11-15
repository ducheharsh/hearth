//! Health, metrics, activity feed and the setup checklist.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use hearth_core::storage::{Role, Status};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::gateway::load_users;
use crate::state::{Point, SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") })) }),
        )
        .route("/system", get(system))
        .route("/events", get(events))
        .route("/setup", get(setup))
}

#[derive(Serialize)]
struct SystemResponse {
    latest: Option<hearth_host::system::SystemSample>,
    history: Vec<Point>,
    display_name: String,
}

async fn system(State(st): State<SharedState>) -> Json<SystemResponse> {
    let m = st.metrics.read().await;
    Json(SystemResponse {
        latest: m.latest.clone(),
        history: m.history.iter().copied().collect(),
        display_name: st.settings.read().await.display_name.clone(),
    })
}

#[derive(Deserialize)]
struct After {
    #[serde(default)]
    after: u64,
}

async fn events(State(st): State<SharedState>, Query(q): Query<After>) -> Json<serde_json::Value> {
    let log = st.events.lock().await;
    Json(json!({ "last": log.last_seq(), "events": log.after(q.after) }))
}

#[derive(Serialize)]
struct Check {
    id: &'static str,
    title: &'static str,
    /// `done`, `todo`, `warn`, `optional`
    state: &'static str,
    detail: String,
    /// Where in the UI to fix it.
    tab: &'static str,
}

/// Everything needed for a fully working server, in order.
#[allow(clippy::too_many_lines)] // a flat, readable checklist
async fn setup(State(st): State<SharedState>) -> Json<Vec<Check>> {
    let ts = st.tailscale.read().await.clone();
    let settings = st.settings.read().await.clone();
    let users = load_users(&st).map_or(0, |u| u.len());
    let vault = {
        let s = st.storage.read().await;
        s.attachments
            .iter()
            .find(|a| a.role == Role::Vault)
            .map(|a| {
                (
                    a.label.clone(),
                    matches!(s.status(a), Status::Online { .. }),
                )
            })
    };
    let c = |id, title, ok: bool, warn: bool, detail: String, tab| Check {
        id,
        title,
        state: if ok {
            "done"
        } else if warn {
            "warn"
        } else {
            "todo"
        },
        detail,
        tab,
    };
    Json(vec![
        c(
            "docker",
            "Docker is running",
            st.docker.is_some(),
            false,
            if st.docker.is_some() {
                "Connected to the Docker engine.".into()
            } else {
                "Start OrbStack (or Docker), then restart hearth.".into()
            },
            "containers",
        ),
        c(
            "tailscale",
            "Tailscale is connected",
            ts.backend_state == "Running",
            ts.installed,
            match ts.backend_state.as_str() {
                "Running" => format!("Connected as {}.", ts.dns_name.unwrap_or_default()),
                "NotInstalled" => "Install Tailscale from tailscale.com/download.".into(),
                other => format!("Tailscale is {other}. Log in from Settings."),
            },
            "settings",
        ),
        c(
            "https",
            "Tailscale HTTPS certificates",
            ts.https_enabled && ts.magic_dns,
            false,
            if ts.https_enabled && ts.magic_dns {
                "MagicDNS and HTTPS are on.".into()
            } else {
                "Turn on MagicDNS and HTTPS Certificates at login.tailscale.com/admin/dns.".into()
            },
            "settings",
        ),
        c(
            "vault",
            "Storage drive attached",
            vault.as_ref().is_some_and(|v| v.1),
            vault.is_some(),
            match vault {
                Some((l, true)) => format!("{l} is online."),
                Some((l, false)) => format!("{l} is attached but not connected."),
                None => "Attach an external SSD as the Vault for photos and documents.".into(),
            },
            "storage",
        ),
        c(
            "login",
            "Login created",
            users > 0,
            false,
            if users > 0 {
                format!("{users} login(s).")
            } else {
                "Create your single sign-on login.".into()
            },
            "settings",
        ),
        c(
            "gateway",
            "Gateway is live",
            settings.gateway_enabled,
            false,
            if settings.gateway_enabled {
                "Apps are published over Tailscale behind your login.".into()
            } else {
                "Turn on the gateway to reach apps from your devices.".into()
            },
            "settings",
        ),
        Check {
            id: "domain",
            title: "Custom domain (optional)",
            state: if settings.domain.is_some() {
                "done"
            } else {
                "optional"
            },
            detail: settings.domain.map_or_else(
                || "Connect a domain with Cloudflare for app.yourdomain.com links.".into(),
                |d| format!("Using {d}."),
            ),
            tab: "settings",
        },
    ])
}
