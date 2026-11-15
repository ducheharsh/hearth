//! Gateway lifecycle: render files, run Caddy + Authelia, publish with `tailscale serve`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context as _, bail};
use hearth_gateway::{
    CaTarget, FUNNEL_PORT, NGROK_PORT, OpenSite, Route, User, render_authelia_config,
    render_ca_sites, render_caddyfile, render_compose, render_open_sites, render_users, routes,
};
use hearth_host::compose::{ComposeEnv, Project, write_private};
use serde::{Deserialize, Serialize};

use crate::events::Level;
use crate::settings::TsCerts;
use crate::state::{AppState, SharedState};
use crate::store;

#[derive(Debug, Serialize)]
pub struct Preview {
    pub routes: Vec<Route>,
    pub open_sites: Vec<OpenSite>,
    pub caddyfile: String,
    pub compose: String,
    pub authelia: String,
    /// Some site uses hearth's certificate.
    pub ca: bool,
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

/// This Mac on `NetBird`, when serving there is on and `NetBird` is connected.
pub async fn netbird_target(st: &AppState) -> Option<CaTarget> {
    if !st.settings.read().await.netbird_enabled {
        return None;
    }
    let s = hearth_host::netbird::status()
        .await
        .ok()
        .filter(|s| s.connected)?;
    Some(CaTarget {
        ip: s.ip?,
        fqdn: s.fqdn?,
    })
}

/// This Mac on the tailnet, when the tailnet uses hearth's certificate (needs MagicDNS:
/// the certificate and the login cookie are for the `.ts.net` name).
pub async fn tailnet_ca_target(st: &AppState) -> Option<CaTarget> {
    if st.settings.read().await.tailscale_certs != TsCerts::Hearth {
        return None;
    }
    let ts = st.tailscale.read().await.clone();
    if ts.backend_state != "Running" {
        return None;
    }
    let ip = ts.ips.iter().find(|ip| ip.contains('.'))?.clone();
    let fqdn = st.config.ts_host.clone().or(ts.dns_name)?;
    Some(CaTarget { ip, fqdn })
}

pub async fn preview(st: &SharedState) -> Preview {
    let (installed, _) = st.installed().await;
    let app_routes = st.app_routes(&installed).await;
    let (domain, tunnels) = {
        let s = st.settings.read().await;
        (s.domain.clone(), s.tunnels.clone())
    };
    let netbird = netbird_target(st).await;
    let targets: Vec<CaTarget> = netbird
        .iter()
        .cloned()
        .chain(tailnet_ca_target(st).await)
        .collect();
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
        caddyfile: render_caddyfile(
            &r,
            domain.as_deref(),
            Some(crate::sessions::log_addr(st).port()),
        ) + &render_open_sites(&open)
            + &render_ca_sites(&r, &targets),
        compose: render_compose(&r, &open, &targets),
        authelia: render_authelia_config(
            &ts,
            domain.as_deref(),
            netbird.as_ref().map(|n| n.fqdn.as_str()),
        ),
        routes: r,
        open_sites: open,
        ca: !targets.is_empty(),
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
    tokio::fs::create_dir_all(d.join("public")).await?;
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

/// Run `redis-cli` in the gateway's Redis (Authelia's sessions).
pub async fn redis(st: &AppState, args: &[&str]) -> anyhow::Result<String> {
    let mut cmd = vec!["redis-cli"];
    cmd.extend_from_slice(args);
    let env = ComposeEnv::default().with("TS_HOST", st.ts_host().await);
    project(st)
        .exec(&env, "redis", &cmd)
        .await
        .context("talking to the gateway's session store")
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

/// Before turning the gateway on: a login exists and the tailnet can serve HTTPS.
async fn precheck(st: &SharedState) -> anyhow::Result<()> {
    if load_users(st)?.is_empty() {
        bail!("Add a user in Settings → Users before turning the gateway on.");
    }
    mark(st, STEP_CHECK, "running", None).await;
    let hearth_ca = st.settings.read().await.tailscale_certs == TsCerts::Hearth;
    if let Ok(ts) = hearth_host::tailscale::status().await
        && ts.backend_state == "Running"
    {
        if hearth_ca && !ts.magic_dns {
            bail!(
                "hearth's certificate is issued for your Mac's MagicDNS name, so MagicDNS must be on. \
                 Turn it on at https://login.tailscale.com/admin/dns, then try again."
            );
        }
        // Without tailnet HTTPS certificates `tailscale serve` can't publish anything (and
        // would stop to ask). Fail fast with the fix instead of starting containers first.
        if !hearth_ca && !ts.https_enabled {
            bail!(
                "Your tailnet doesn't have HTTPS certificates turned on, so Tailscale can't publish apps. \
                 Turn on MagicDNS and HTTPS Certificates at https://login.tailscale.com/admin/dns, \
                 or switch to hearth's own certificate (Settings → Network → Tailscale)."
            );
        }
    }
    let note = hearth_ca.then(|| "Using hearth's own certificate.".to_owned());
    mark(st, STEP_CHECK, "done", note).await;
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
    // With hearth's certificate Caddy binds the Tailscale IP itself: stop `tailscale serve`
    // from intercepting those ports first.
    let hearth_ca = tailnet_ca_target(st).await.is_some();
    let all: BTreeSet<u16> = p.routes.iter().map(|r| r.port).collect();
    let wanted = if hearth_ca {
        BTreeSet::new()
    } else {
        all.clone()
    };
    unserve_stale(st, &wanted).await?;
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
    if p.ca {
        publish_ca_cert(st).await;
    }
    mark(st, STEP_START, "done", None).await;
    mark(st, STEP_PUBLISH, "running", None).await;
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
    store::save(
        &dir(st).join("served.json"),
        &Served {
            ports: wanted.clone(),
        },
    )?;

    // Tailscale Funnel follows the settings.
    if p.open_sites.iter().any(|o| o.listen == FUNNEL_PORT) {
        hearth_host::tailscale::funnel(FUNNEL_PORT)
            .await
            .context("tailscale funnel (is Funnel allowed in your tailnet policy?)")?;
    } else {
        let _ = hearth_host::tailscale::funnel_off().await;
    }
    let detail = if hearth_ca {
        format!("{} sites, with hearth's certificate", all.len())
    } else {
        format!("{} sites", all.len())
    };
    mark(st, STEP_PUBLISH, "done", Some(detail)).await;
    Ok(Outcome {
        applied: true,
        served: all.into_iter().collect(),
        note: "Gateway is live.".into(),
    })
}

/// Stop serving ports hearth published before that aren't wanted now.
async fn unserve_stale(st: &AppState, wanted: &BTreeSet<u16>) -> anyhow::Result<()> {
    let path = dir(st).join("served.json");
    let mut served: Served = store::load(&path)?;
    for port in served.ports.difference(wanted).copied().collect::<Vec<_>>() {
        if let Err(e) = hearth_host::tailscale::unserve(port).await {
            tracing::warn!(port, error = %e, "unserve failed");
        }
        served.ports.remove(&port);
    }
    store::save(&path, &served)
}

/// Copy the CA's public certificate to where Caddy serves it for download (`CERT_PORT`).
/// Caddy creates the CA on first start, so wait for it briefly. Only the certificate is
/// copied; the private key never leaves `caddy-data`.
async fn publish_ca_cert(st: &AppState) {
    let src = ca_cert_path(st);
    for _ in 0..20 {
        if let Ok(bytes) = tokio::fs::read(&src).await {
            if let Err(e) = tokio::fs::write(dir(st).join("public/hearth-ca.crt"), bytes).await {
                tracing::warn!(error = %e, "could not publish the CA certificate");
            }
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    tracing::warn!("Caddy has not created its CA yet; certificate download unavailable");
}

/// Caddy's internal root CA (hearth's certificate, trusted once per device).
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

/// On startup: when the gateway is on but hearth now renders it differently (after an
/// upgrade), apply once so new pieces (like the session store) take effect.
pub async fn reconcile(st: SharedState) {
    // Let the Tailscale poller fill in the machine name first.
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    if !st.settings.read().await.gateway_enabled || st.ts_host().await == "localhost" {
        return;
    }
    let p = preview(&st).await;
    let d = dir(&st);
    let same =
        |file: &str, body: &str| std::fs::read_to_string(d.join(file)).is_ok_and(|x| x == body);
    if same("Caddyfile", &p.caddyfile)
        && same("compose.yml", &p.compose)
        && same("authelia/configuration.yml", &p.authelia)
    {
        return;
    }
    tracing::info!("gateway configuration changed since last start; re-applying");
    let _ops = st.ops.lock().await;
    reapply_quietly(&st).await;
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
