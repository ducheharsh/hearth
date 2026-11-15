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
        .route("/access", get(access))
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
/// Stays awake on power: the sleep assertion is held and the Mac is plugged in.
async fn awake_check(st: &SharedState, keep_awake: bool) -> Check {
    let holding = st.always_on.lock().await.holding();
    let power = hearth_host::power::status().await.ok();
    let on_ac = power.as_ref().is_none_or(|p| p.on_ac);
    let laptop = power
        .as_ref()
        .is_some_and(hearth_host::power::PowerStatus::is_laptop);
    let lid_ok = power.as_ref().is_some_and(|p| p.sleep_disabled);
    let (state, detail) = match (keep_awake, holding, on_ac) {
        (false, ..) => (
            "todo",
            "Turn on Keep Awake so the Mac doesn't sleep while serving.".to_owned(),
        ),
        (true, true, true) if laptop && !lid_ok => (
            "done",
            "Awake on power. Keep the lid open, or allow lid-closed in Settings → Always On."
                .to_owned(),
        ),
        (true, true, true) => ("done", "Awake on power.".to_owned()),
        (true, _, false) => (
            "warn",
            "On battery: plug in so the Mac can stay awake.".to_owned(),
        ),
        (true, false, true) => ("warn", "Starting the keep-awake assertion…".to_owned()),
    };
    Check {
        id: "awake",
        title: "Stays awake",
        state,
        detail,
        tab: "settings",
    }
}

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
        {
            let own = settings.tailscale_certs == crate::settings::TsCerts::Hearth;
            let ok = ts.magic_dns && (ts.https_enabled || own);
            c(
                "https",
                "Tailscale HTTPS certificates",
                ok,
                false,
                match (ok, own) {
                    (true, true) => {
                        "Using hearth’s own certificate. Install it once on each device.".into()
                    }
                    (true, false) => "MagicDNS and HTTPS are on.".into(),
                    (false, true) => {
                        "Turn on MagicDNS at login.tailscale.com/admin/dns (hearth’s certificate is for that name).".into()
                    }
                    (false, false) => "Turn on MagicDNS and HTTPS Certificates at login.tailscale.com/admin/dns, or use hearth’s own certificate.".into(),
                },
                "settings",
            )
        },
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
        awake_check(&st, settings.always_on.keep_awake).await,
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

/// Every address that reaches the dashboard right now, plus QR codes for the best one (and
/// for installing hearth's certificate, when it uses one).
async fn access(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let (gateway, domain) = {
        let s = st.settings.read().await;
        (s.gateway_enabled, s.domain.clone())
    };
    let ts = {
        let t = st.tailscale.read().await;
        (t.backend_state == "Running")
            .then(|| t.dns_name.clone())
            .flatten()
    };
    let ts = st.config.ts_host.clone().or(ts);
    let netbird = crate::gateway::netbird_target(&st).await;
    let ts_ca = crate::gateway::tailnet_ca_target(&st).await;
    let local = format!("http://{}", st.config.listen);
    let links = hearth_gateway::access_links(&hearth_gateway::Reach {
        gateway_enabled: gateway,
        ts_host: ts.as_deref(),
        ts_ca_ip: ts_ca.as_ref().map(|t| t.ip.as_str()),
        domain: domain.as_deref(),
        netbird: netbird.as_ref(),
        local: &local,
    });
    let qr = |url: &str| {
        qrcode::QrCode::new(url.as_bytes()).ok().map(|q| {
            q.render::<qrcode::render::svg::Color<'_>>()
                .min_dimensions(176, 176)
                .quiet_zone(true)
                .build()
        })
    };
    let best = links.first().filter(|l| l.kind != "local");
    let trust_qr = best.and_then(|l| l.trust_url.as_deref()).and_then(qr);
    let qr = best.and_then(|l| qr(&l.url));
    Json(serde_json::json!({
        "gateway_enabled": gateway,
        "links": links,
        "qr_svg": qr,
        "trust_qr_svg": trust_qr,
    }))
}
