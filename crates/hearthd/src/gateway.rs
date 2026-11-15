//! Gateway lifecycle: render files, run Caddy + Authelia, publish with `tailscale serve`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context as _, bail};
use hearth_gateway::{
    FUNNEL_PORT, NGROK_PORT, NetbirdTarget, OpenSite, Route, User, render_authelia_config,
    render_caddyfile, render_compose, render_netbird_sites, render_open_sites, render_users,
    routes,
};
use hearth_host::compose::{ComposeEnv, Project, write_private};
use serde::{Deserialize, Serialize};

use crate::events::Level;
use crate::state::{AppState, SharedState};
use crate::store;

#[derive(Debug, Serialize)]
pub struct Preview {
    pub routes: Vec<Route>,
    pub open_sites: Vec<OpenSite>,
    pub caddyfile: String,
    pub compose: String,
    pub authelia: String,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub applied: bool,
    pub served: Vec<u16>,
    pub note: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Served {
    ports: BTreeSet<u16>,
}

fn dir(st: &AppState) -> PathBuf {
    st.config.gateway_dir()
}

fn project(st: &AppState) -> Project {
    let d = dir(st);
    Project {
        name: "hearth-gateway".into(),
        files: vec![d.join("compose.yml")],
        dir: d,
    }
}

pub fn load_users(st: &AppState) -> anyhow::Result<Vec<User>> {
    store::load(&dir(st).join("users.json"))
}

pub async fn preview(st: &SharedState) -> Preview {
    let (installed, _) = st.installed().await;
    let app_routes = st.app_routes(&installed).await;
    let (domain, tunnels, nb_on) = {
        let s = st.settings.read().await;
        (s.domain.clone(), s.tunnels.clone(), s.netbird_enabled)
    };
    let netbird = if nb_on {
        hearth_host::netbird::status()
            .await
            .ok()
            .filter(|s| s.connected)
            .and_then(|s| {
                Some(NetbirdTarget {
                    ip: s.ip?,
                    fqdn: s.fqdn?,
                })
            })
    } else {
        None
    };
    let upstream = |id: &Option<String>| {
        id.as_ref()
            .and_then(|id| app_routes.iter().find(|r| &r.id == id))
            .map(|r| (r.name.clone(), r.upstream.clone()))
    };
    let mut open = Vec::new();
    if let Some((name, up)) = upstream(&tunnels.funnel_app) {
        open.push(OpenSite {
            name: format!("{name} via Funnel"),
            listen: FUNNEL_PORT,
            upstream: up,
            publish: true,
        });
    }
    if let Some((name, up)) = upstream(&tunnels.ngrok_app) {
        open.push(OpenSite {
            name: format!("{name} via ngrok"),
            listen: NGROK_PORT,
            upstream: up,
            publish: false,
        });
    }
    let mut r = routes(app_routes.clone());
    // Roles and per-app access become Caddy rules on each route.
    if let Ok(users) = load_users(st) {
        hearth_gateway::apply_access(&mut r, &users);
    }
    let ts = st.ts_host().await;
    Preview {
        caddyfile: render_caddyfile(&r, domain.as_deref())
            + &render_open_sites(&open)
            + &netbird
                .as_ref()
                .map(|nb| render_netbird_sites(&r, nb))
                .unwrap_or_default(),
        compose: render_compose(&r, &open, netbird.as_ref()),
        authelia: render_authelia_config(
            &ts,
            domain.as_deref(),
            netbird.as_ref().map(|n| n.fqdn.as_str()),
        ),
        routes: r,
        open_sites: open,
    }
}

/// `authelia.env` secrets, generated once.
async fn ensure_authelia_env(st: &AppState) -> anyhow::Result<()> {
    let p = dir(st).join("authelia.env");
    if p.exists() {
        return Ok(());
    }
    let body = format!(
        "AUTHELIA_SESSION_SECRET={}\nAUTHELIA_STORAGE_ENCRYPTION_KEY={}\nAUTHELIA_IDENTITY_VALIDATION_RESET_PASSWORD_JWT_SECRET={}\n",
        hearth_host::random_hex(32)?,
        hearth_host::random_hex(32)?,
        hearth_host::random_hex(32)?
    );
    write_private(&p, body.as_bytes()).await?;
    Ok(())
}

/// Write all gateway files (always safe; nothing is exposed by this alone).
pub async fn write(st: &SharedState) -> anyhow::Result<Preview> {
    let p = preview(st).await;
    let d = dir(st);
    tokio::fs::create_dir_all(d.join("authelia")).await?;
    tokio::fs::write(d.join("Caddyfile"), &p.caddyfile).await?;
    tokio::fs::write(d.join("compose.yml"), &p.compose).await?;
    tokio::fs::write(d.join("authelia/configuration.yml"), &p.authelia).await?;
    write_private(
        &d.join("authelia/users.yml"),
        render_users(&load_users(st)?).as_bytes(),
    )
    .await?;
    ensure_authelia_env(st).await?;
    Ok(p)
}

/// After logins or access change: rewrite the files (Authelia watches its users file) and,
/// if the gateway runs, reload Caddy so access rules apply immediately.
pub async fn refresh(st: &SharedState) -> anyhow::Result<()> {
    write(st).await?;
    if st.settings.read().await.gateway_enabled {
        let env = ComposeEnv::default().with("TS_HOST", st.ts_host().await);
        project(st)
            .exec(
                &env,
                "caddy",
                &["caddy", "reload", "--config", "/etc/caddy/Caddyfile"],
            )
            .await
            .context("reloading Caddy with the new access rules")?;
    }
    Ok(())
}

/// Progress of turning the gateway on, shown step by step in Settings.
#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub started: u64,
    pub finished: Option<u64>,
    pub ok: bool,
    pub steps: Vec<JobStep>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobStep {
    pub label: &'static str,
    /// `pending` | `running` | `done` | `failed`
    pub state: &'static str,
    pub detail: Option<String>,
}

pub const STEP_CHECK: usize = 0;
pub const STEP_WRITE: usize = 1;
pub const STEP_PULL: usize = 2;
pub const STEP_START: usize = 3;
pub const STEP_PUBLISH: usize = 4;

#[must_use]
pub fn new_job() -> Job {
    let step = |label| JobStep {
        label,
        state: "pending",
        detail: None,
    };
    Job {
        started: crate::events::now(),
        finished: None,
        ok: false,
        steps: vec![
            step("Check your tailnet"),
            step("Write the gateway configuration"),
            step("Download Caddy and Authelia"),
            step("Start the login gateway"),
            step("Publish on Tailscale"),
        ],
    }
}

/// Update a step of the running job; no-op when no enable job is in progress
/// (apply also runs after every install).
pub async fn mark(st: &SharedState, i: usize, state: &'static str, detail: Option<String>) {
    let mut job = st.gateway_job.write().await;
    if let Some(s) = job
        .as_mut()
        .filter(|j| j.finished.is_none())
        .and_then(|j| j.steps.get_mut(i))
    {
        s.state = state;
        if detail.is_some() || state == "done" {
            s.detail = detail;
        }
    }
}

async fn job_active(st: &SharedState) -> bool {
    st.gateway_job
        .read()
        .await
        .as_ref()
        .is_some_and(|j| j.finished.is_none())
}

/// Before turning the gateway on: a login exists and the tailnet can issue HTTPS certificates.
async fn precheck(st: &SharedState) -> anyhow::Result<()> {
    if load_users(st)?.is_empty() {
        bail!("Add a user in Settings → Users before turning the gateway on.");
    }
    mark(st, STEP_CHECK, "running", None).await;
    // Without tailnet HTTPS certificates `tailscale serve` can't publish anything (and
    // would stop to ask). Fail fast with the fix instead of starting containers first.
    if let Ok(ts) = hearth_host::tailscale::status().await
        && ts.backend_state == "Running"
        && !ts.https_enabled
    {
        bail!(
            "Your tailnet doesn't have HTTPS certificates turned on, so Tailscale can't publish apps. \
             Turn on MagicDNS and HTTPS Certificates at https://login.tailscale.com/admin/dns, then try again."
        );
    }
    mark(st, STEP_CHECK, "done", None).await;
    Ok(())
}

/// Apply: write files; if enabled, run the gateway and publish every route.
pub async fn apply(st: &SharedState) -> anyhow::Result<Outcome> {
    let enabled = st.settings.read().await.gateway_enabled;
    if enabled {
        precheck(st).await?;
    }
    mark(st, STEP_WRITE, "running", None).await;
    let p = write(st).await?;
    mark(st, STEP_WRITE, "done", None).await;
    if !enabled {
        return Ok(Outcome {
            applied: false,
            served: vec![],
            note: "Gateway is off. Files are written; turn it on in Settings to publish.".into(),
        });
    }
    let env = ComposeEnv::default().with("TS_HOST", st.ts_host().await);
    let proj = project(st);
    if job_active(st).await {
        // Pull separately so the slow first download shows as its own step.
        mark(
            st,
            STEP_PULL,
            "running",
            Some("First time takes a minute or two.".into()),
        )
        .await;
        proj.pull(&env)
            .await
            .context("downloading Caddy + Authelia")?;
        mark(st, STEP_PULL, "done", None).await;
    }
    mark(st, STEP_START, "running", None).await;
    proj.up(&env).await.context("starting Caddy + Authelia")?;
    // Routes may have changed without the compose file changing: reload Caddy's config.
    if let Err(e) = proj
        .exec(
            &env,
            "caddy",
            &["caddy", "reload", "--config", "/etc/caddy/Caddyfile"],
        )
        .await
    {
        tracing::warn!(error = %e, "caddy reload failed (it may still be starting)");
    }

    mark(st, STEP_START, "done", None).await;
    mark(st, STEP_PUBLISH, "running", None).await;
    let served_path = dir(st).join("served.json");
    let mut served: Served = store::load(&served_path)?;
    let wanted: BTreeSet<u16> = p.routes.iter().map(|r| r.port).collect();
    for port in served
        .ports
        .difference(&wanted)
        .copied()
        .collect::<Vec<_>>()
    {
        if let Err(e) = hearth_host::tailscale::unserve(port).await {
            tracing::warn!(port, error = %e, "unserve failed");
        }
    }
    for (i, port) in wanted.iter().enumerate() {
        mark(
            st,
            STEP_PUBLISH,
            "running",
            Some(format!("Port {port} ({} of {})", i + 1, wanted.len())),
        )
        .await;
        hearth_host::tailscale::serve(*port)
            .await
            .with_context(|| format!("tailscale serve :{port}"))?;
    }
    served.ports = wanted.clone();
    store::save(&served_path, &served)?;

    // Tailscale Funnel follows the settings.
    if p.open_sites.iter().any(|o| o.listen == FUNNEL_PORT) {
        hearth_host::tailscale::funnel(FUNNEL_PORT)
            .await
            .context("tailscale funnel (is Funnel allowed in your tailnet policy?)")?;
    } else {
        let _ = hearth_host::tailscale::funnel_off().await;
    }
    mark(
        st,
        STEP_PUBLISH,
        "done",
        Some(format!("{} sites", wanted.len())),
    )
    .await;
    Ok(Outcome {
        applied: true,
        served: wanted.into_iter().collect(),
        note: "Gateway is live.".into(),
    })
}

/// Caddy's internal root CA (for trusting NetBird HTTPS on devices).
pub fn ca_cert_path(st: &AppState) -> PathBuf {
    dir(st).join("caddy-data/caddy/pki/authorities/local/root.crt")
}

/// Stop the gateway and unpublish everything hearth published.
pub async fn disable(st: &SharedState) -> anyhow::Result<()> {
    let served_path = dir(st).join("served.json");
    let served: Served = store::load(&served_path)?;
    for port in served.ports {
        let _ = hearth_host::tailscale::unserve(port).await;
    }
    let _ = hearth_host::tailscale::funnel_off().await;
    store::save(&served_path, &Served::default())?;
    if dir(st).join("compose.yml").exists() {
        project(st)
            .down(&ComposeEnv::default().with("TS_HOST", st.ts_host().await))
            .await?;
    }
    st.event(
        Level::Info,
        "gateway",
        "Gateway stopped",
        "Nothing is published over Tailscale anymore.",
    )
    .await;
    Ok(())
}

/// Re-apply after a change, logging failures as events instead of failing the caller.
pub async fn reapply_quietly(st: &SharedState) {
    if let Err(e) = apply(st).await {
        st.event(
            Level::Error,
            "gateway",
            "Gateway update failed",
            format!("{e:#}"),
        )
        .await;
    }
}
