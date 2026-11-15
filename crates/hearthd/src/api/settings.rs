//! Settings page: general settings, Tailscale, gateway + logins, Cloudflare, tunnels.

use axum::extract::{Path, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use super::bad;
use crate::error::{ApiError, internal};
use crate::events::Level;
use crate::settings::Settings;
use crate::state::SharedState;
use crate::{cloudflare, gateway, headscale, store, tunnels};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/settings", get(get_settings).put(put_settings))
        .route("/tailscale", get(ts_status))
        .route("/tailscale/login", post(ts_login))
        .route("/tailscale/logout", post(ts_logout))
        .route("/tailscale/up", post(|s| ts_updown(s, true)))
        .route("/tailscale/down", post(|s| ts_updown(s, false)))
        .route("/tailscale/hostname", put(ts_hostname))
        .route("/tailscale/serve", get(ts_serve))
        .route("/tailscale/serve/reset", post(ts_serve_reset))
        .route("/gateway", get(gw_preview))
        .route("/gateway/enable", post(gw_enable))
        .route("/gateway/disable", post(gw_disable))
        .route("/gateway/apply", post(gw_apply))
        .route("/gateway/users", get(users).post(add_user))
        .route("/gateway/users/{name}", delete(delete_user))
        .route(
            "/gateway/users/{name}/totp",
            post(totp_start).delete(totp_remove),
        )
        .route("/gateway/users/{name}/totp/verify", post(totp_verify))
        .route("/cloudflare", get(cf_status))
        .route("/cloudflare/connect", post(cf_connect))
        .route("/cloudflare/disconnect", post(cf_disconnect))
        .route("/tunnels/funnel", put(funnel))
        .route("/tunnels/ngrok", put(ngrok))
        .route("/tailscale/login-server", post(ts_login_server))
        .route("/netbird", get(nb_status))
        .route("/netbird/up", post(nb_up))
        .route("/netbird/down", post(nb_down))
        .route("/netbird/expose", put(nb_expose))
        .route("/gateway/ca.crt", get(ca_cert))
        .route("/headscale", get(hs_status).delete(hs_uninstall))
        .route("/headscale/install", post(hs_install))
        .route("/headscale/users", post(hs_user))
        .route("/headscale/keys", post(hs_key))
        .route("/headscale/join-self", post(hs_join_self))
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(err)`
fn err(e: anyhow::Error) -> ApiError {
    bad(format!("{e:#}"))
}

async fn get_settings(State(st): State<SharedState>) -> Json<Settings> {
    Json(st.settings.read().await.clone())
}

/// Replace general settings. Gateway, domain, Cloudflare ids and tunnels are managed by
/// their own endpoints, so they're kept from the current value.
async fn put_settings(
    State(st): State<SharedState>,
    Json(mut next): Json<Settings>,
) -> Result<Json<Settings>, ApiError> {
    let changed_discover;
    {
        let mut cur = st.settings.write().await;
        next.gateway_enabled = cur.gateway_enabled;
        next.domain.clone_from(&cur.domain);
        next.cloudflare = cur.cloudflare.clone();
        next.tunnels = cur.tunnels.clone();
        next.apps.clone_from(&cur.apps);
        next.validate().map_err(bad)?;
        store::save(&st.config.state_dir.join("settings.json"), &next).map_err(internal)?;
        changed_discover = next.discover_enabled && !cur.discover_enabled;
        *cur = next.clone();
    }
    if changed_discover {
        let s = st.clone();
        tokio::spawn(async move {
            let _ = crate::discover::sync(&s).await;
        });
    }
    st.event(Level::Success, "settings", "Settings saved", "")
        .await;
    Ok(Json(next))
}

async fn ts_status(State(st): State<SharedState>) -> Json<hearth_host::tailscale::TailscaleStatus> {
    if let Ok(s) = hearth_host::tailscale::status().await {
        *st.tailscale.write().await = s;
    }
    Json(st.tailscale.read().await.clone())
}

async fn ts_login() -> Json<serde_json::Value> {
    hearth_host::tailscale::start_login();
    Json(json!({ "ok": true, "hint": "A login link appears in a few seconds." }))
}

async fn ts_logout(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    hearth_host::tailscale::logout().await.map_err(bad)?;
    st.event(Level::Warning, "tailscale", "Logged out of Tailscale", "")
        .await;
    Ok(Json(json!({ "ok": true })))
}

async fn ts_updown(
    State(st): State<SharedState>,
    up: bool,
) -> Result<Json<serde_json::Value>, ApiError> {
    hearth_host::tailscale::set_up(up).await.map_err(bad)?;
    st.event(
        Level::Info,
        "tailscale",
        if up {
            "Tailscale connected"
        } else {
            "Tailscale disconnected"
        },
        "",
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct Hostname {
    hostname: String,
}

async fn ts_hostname(
    State(st): State<SharedState>,
    Json(h): Json<Hostname>,
) -> Result<Json<serde_json::Value>, ApiError> {
    hearth_host::tailscale::set_hostname(h.hostname.trim())
        .await
        .map_err(bad)?;
    st.event(
        Level::Success,
        "tailscale",
        "Machine name changed",
        format!("Now {}. App links update in a moment.", h.hostname.trim()),
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

async fn ts_serve() -> Result<Json<serde_json::Value>, ApiError> {
    let m = hearth_host::tailscale::serve_status().await.map_err(bad)?;
    Ok(Json(json!(m)))
}

async fn ts_serve_reset(
    State(st): State<SharedState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    hearth_host::tailscale::serve_reset().await.map_err(bad)?;
    st.event(
        Level::Warning,
        "tailscale",
        "Published ports cleared",
        "Re-apply the gateway to publish apps again.",
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

async fn gw_preview(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let p = gateway::preview(&st).await;
    let users = gateway::load_users(&st).map_or(0, |u| u.len());
    let s = st.settings.read().await;
    Json(json!({ "enabled": s.gateway_enabled, "users": users, "preview": p }))
}

async fn set_gateway(st: &SharedState, on: bool) -> Result<(), ApiError> {
    let mut s = st.settings.write().await;
    s.gateway_enabled = on;
    store::save(&st.config.state_dir.join("settings.json"), &*s).map_err(internal)
}

async fn gw_enable(State(st): State<SharedState>) -> Result<Json<gateway::Outcome>, ApiError> {
    let _ops = st.ops.lock().await;
    set_gateway(&st, true).await?;
    match gateway::apply(&st).await {
        Ok(o) => {
            st.event(
                Level::Success,
                "gateway",
                "Gateway is live",
                format!("{} sites published over Tailscale.", o.served.len()),
            )
            .await;
            Ok(Json(o))
        }
        Err(e) => {
            set_gateway(&st, false).await?;
            Err(err(e))
        }
    }
}

async fn gw_disable(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    set_gateway(&st, false).await?;
    gateway::disable(&st).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn gw_apply(State(st): State<SharedState>) -> Result<Json<gateway::Outcome>, ApiError> {
    let _ops = st.ops.lock().await;
    Ok(Json(gateway::apply(&st).await.map_err(err)?))
}

async fn users(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let list: Vec<_> = gateway::load_users(&st)
        .map_err(internal)?
        .into_iter()
        .map(
            |u| json!({ "username": u.username, "display_name": u.display_name, "email": u.email }),
        )
        .collect();
    Ok(Json(json!(list)))
}

#[derive(Deserialize)]
struct NewUser {
    username: String,
    #[serde(default)]
    display_name: String,
    email: String,
    password: String,
}

async fn add_user(
    State(st): State<SharedState>,
    Json(u): Json<NewUser>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let salt: [u8; 16] = hearth_host::random_bytes().map_err(internal)?;
    let username = u.username.trim().to_lowercase();
    // argon2 at 64 MiB takes ~100 ms: keep it off the async workers.
    let user = tokio::task::spawn_blocking(move || {
        hearth_gateway::new_user(
            &username,
            &u.display_name,
            u.email.trim(),
            &u.password,
            &salt,
        )
    })
    .await
    .map_err(internal)?
    .map_err(bad)?;
    let path = st.config.gateway_dir().join("users.json");
    let mut list = gateway::load_users(&st).map_err(internal)?;
    let replaced = list.iter().any(|x| x.username == user.username);
    // Changing a password keeps an existing authenticator enrollment.
    let mut user = user;
    user.totp = list.iter().any(|x| x.username == user.username && x.totp);
    list.retain(|x| x.username != user.username);
    let name = user.username.clone();
    list.push(user);
    store::save(&path, &list).map_err(internal)?;
    gateway::write(&st).await.map_err(err)?;
    st.event(
        Level::Success,
        "gateway",
        if replaced {
            "Password changed"
        } else {
            "Login created"
        },
        name,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_user(
    State(st): State<SharedState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut list = gateway::load_users(&st).map_err(internal)?;
    if list.len() == 1 && list[0].username == name && st.settings.read().await.gateway_enabled {
        return Err(bad(
            "You can't delete the last login while the gateway is on.",
        ));
    }
    list.retain(|x| x.username != name);
    store::save(&st.config.gateway_dir().join("users.json"), &list).map_err(internal)?;
    gateway::write(&st).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn cf_status(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let s = st.settings.read().await;
    let has_token = cloudflare::load_secrets(&st).is_ok_and(|x| x.api_token.is_some());
    let running = match &st.docker {
        Some(d) => d.containers().await.is_ok_and(|c| {
            c.iter()
                .any(|c| c.project.as_deref() == Some("hearth-cloudflared") && c.state == "running")
        }),
        None => false,
    };
    Json(json!({
        "domain": s.domain, "enabled": s.cloudflare.enabled, "tunnel_id": s.cloudflare.tunnel_id,
        "has_token": has_token, "connector_running": running,
        "public_apps": s.apps.iter().filter(|(_, o)| o.public).map(|(k, _)| k).collect::<Vec<_>>(),
    }))
}

#[derive(Deserialize)]
struct Connect {
    api_token: String,
    domain: String,
}

async fn cf_connect(
    State(st): State<SharedState>,
    Json(c): Json<Connect>,
) -> Result<Json<cloudflare::Connected>, ApiError> {
    let _ops = st.ops.lock().await;
    Ok(Json(
        cloudflare::connect(&st, &c.api_token, &c.domain)
            .await
            .map_err(err)?,
    ))
}

#[derive(Deserialize)]
struct Disconnect {
    #[serde(default)]
    delete: bool,
}

async fn cf_disconnect(
    State(st): State<SharedState>,
    Json(d): Json<Disconnect>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    cloudflare::disconnect(&st, d.delete).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct Funnel {
    app: Option<String>,
}

async fn funnel(
    State(st): State<SharedState>,
    Json(f): Json<Funnel>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    tunnels::set_funnel(&st, f.app).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct Ngrok {
    app: Option<String>,
    domain: Option<String>,
    authtoken: Option<String>,
}

async fn ngrok(
    State(st): State<SharedState>,
    Json(n): Json<Ngrok>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    tunnels::set_ngrok(&st, n.app, n.domain, n.authtoken)
        .await
        .map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct LoginServer {
    login_server: String,
    auth_key: String,
}

/// Point this machine's Tailscale at a custom control server (Headscale).
async fn ts_login_server(
    State(st): State<SharedState>,
    Json(l): Json<LoginServer>,
) -> Result<Json<serde_json::Value>, ApiError> {
    hearth_host::tailscale::login_with(l.login_server.trim(), l.auth_key.trim())
        .await
        .map_err(|e| bad(format!("Login failed: {e}")))?;
    st.event(
        Level::Success,
        "tailscale",
        "Joined custom control server",
        l.login_server,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

async fn nb_status() -> Json<hearth_host::netbird::NetbirdStatus> {
    Json(hearth_host::netbird::status().await.unwrap_or_default())
}

#[derive(Deserialize)]
struct NbUp {
    setup_key: String,
    management_url: Option<String>,
}

async fn nb_up(
    State(st): State<SharedState>,
    Json(u): Json<NbUp>,
) -> Result<Json<serde_json::Value>, ApiError> {
    hearth_host::netbird::up(&u.setup_key, u.management_url.as_deref())
        .await
        .map_err(bad)?;
    st.event(Level::Success, "netbird", "NetBird connected", "")
        .await;
    Ok(Json(json!({ "ok": true })))
}

async fn nb_down(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    hearth_host::netbird::down().await.map_err(bad)?;
    st.event(Level::Info, "netbird", "NetBird disconnected", "")
        .await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct Expose {
    enabled: bool,
}

/// Serve the gateway on this machine's NetBird address too.
async fn nb_expose(
    State(st): State<SharedState>,
    Json(e): Json<Expose>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    if e.enabled
        && !hearth_host::netbird::status()
            .await
            .is_ok_and(|s| s.connected)
    {
        return Err(bad("Connect NetBird first."));
    }
    {
        let mut s = st.settings.write().await;
        s.netbird_enabled = e.enabled;
        store::save(&st.config.state_dir.join("settings.json"), &*s).map_err(internal)?;
    }
    gateway::reapply_quietly(&st).await;
    Ok(Json(json!({ "ok": true })))
}

/// Download Caddy's internal root CA so devices trust NetBird HTTPS.
async fn ca_cert(State(st): State<SharedState>) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse as _;
    let bytes = tokio::fs::read(gateway::ca_cert_path(&st))
        .await
        .map_err(|_| {
            ApiError::NotFound("CA certificate (turn on the gateway with NetBird first)".into())
        })?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-x509-ca-cert"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"hearth-local-ca.crt\"",
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn hs_status(State(st): State<SharedState>) -> Json<headscale::Status> {
    Json(headscale::status(&st).await)
}

async fn hs_install(
    State(st): State<SharedState>,
    Json(c): Json<headscale::HeadscaleConfig>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _ops = st.ops.lock().await;
    headscale::install(&st, c).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn hs_uninstall(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    headscale::uninstall(&st).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct HsUser {
    name: String,
}

async fn hs_user(
    State(st): State<SharedState>,
    Json(u): Json<HsUser>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = u.name.trim().to_lowercase();
    if !crate::settings::valid_label(&name) {
        return Err(bad("User names are lowercase letters, digits and '-'."));
    }
    Ok(Json(
        headscale::cli(&st, &["users", "create", &name])
            .await
            .map_err(err)?,
    ))
}

#[derive(Deserialize)]
struct HsKey {
    /// Numeric user id.
    user: u64,
    #[serde(default)]
    reusable: bool,
    #[serde(default = "default_hours")]
    hours: u32,
}
const fn default_hours() -> u32 {
    24
}

async fn hs_key(
    State(st): State<SharedState>,
    Json(k): Json<HsKey>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let user = k.user.to_string();
    let exp = format!("{}h", k.hours.clamp(1, 24 * 90));
    let mut args = vec![
        "preauthkeys",
        "create",
        "--user",
        user.as_str(),
        "--expiration",
        exp.as_str(),
    ];
    if k.reusable {
        args.push("--reusable");
    }
    Ok(Json(headscale::cli(&st, &args).await.map_err(err)?))
}

#[derive(Deserialize)]
struct JoinSelf {
    auth_key: String,
}

/// Move this machine's Tailscale onto the local Headscale.
async fn hs_join_self(
    State(st): State<SharedState>,
    Json(j): Json<JoinSelf>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let cfg = headscale::load(&st)
        .map_err(internal)?
        .ok_or_else(|| bad("Install Headscale first."))?;
    let url = format!("https://{}", cfg.hostname);
    hearth_host::tailscale::login_with(&url, j.auth_key.trim())
        .await
        .map_err(|e| bad(format!("Join failed: {e}")))?;
    st.event(
        Level::Success,
        "headscale",
        "This machine joined Headscale",
        url,
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

async fn totp_start(
    State(st): State<SharedState>,
    Path(name): Path<String>,
) -> Result<Json<crate::twofa::Enrollment>, ApiError> {
    Ok(Json(crate::twofa::start(&st, &name).await.map_err(err)?))
}

#[derive(Deserialize)]
struct TotpVerify {
    secret: String,
    code: String,
}

async fn totp_verify(
    State(st): State<SharedState>,
    Path(name): Path<String>,
    Json(v): Json<TotpVerify>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::twofa::verify(&st, &name, &v.secret, &v.code)
        .await
        .map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}

async fn totp_remove(
    State(st): State<SharedState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::twofa::remove(&st, &name).await.map_err(err)?;
    Ok(Json(json!({ "ok": true })))
}
