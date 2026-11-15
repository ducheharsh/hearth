//! Marketplace apps: catalog, install (with config form), lifecycle, overrides, custom apps,
//! ratings and logos.

use std::collections::BTreeMap;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use hearth_core::capacity::{self, Fit, Verdict};
use hearth_core::compose_config::{self, AppConfig, ServiceConfig};
use hearth_core::{AppManifest, discover::Entry};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::bad;
use crate::error::{ApiError, internal};
use crate::events::Level;
use crate::settings::AppOverride;
use crate::state::SharedState;
use crate::{gateway, store};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/catalog", get(catalog))
        .route("/apps/{id}/install", post(install))
        .route("/apps/{id}/remove", post(remove))
        .route("/apps/{id}/{action}", post(lifecycle))
        .route("/apps/{id}/config", get(get_config))
        .route("/apps/{id}/override", put(put_override))
        .route("/custom-apps", post(create_custom))
        .route("/custom-apps/{id}", axum::routing::delete(delete_custom))
        .route("/ratings/{key}", put(put_rating))
        .route("/icons/{file}", get(icon))
        .route("/apps/{id}/history", get(history))
}

#[derive(Serialize)]
struct Stats {
    stars: Option<u64>,
    updated_at: Option<String>,
    release: Option<String>,
    commits_this_month: u32,
    licenses: Vec<String>,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)] // flat JSON for the UI
struct CatalogEntry {
    id: String,
    name: String,
    description: String,
    category: String,
    website: Option<String>,
    repo: Option<String>,
    featured: bool,
    editors_note: Option<String>,
    icon: Option<String>,
    port: u16,
    url: String,
    domain_url: Option<String>,
    subdomain: String,
    public: bool,
    ram_mb: u64,
    needs_vault: bool,
    roles: Vec<&'static str>,
    protected: bool,
    custom: bool,
    installed: bool,
    running: bool,
    verdict: Option<Verdict>,
    stats: Option<Stats>,
    rating: Option<u8>,
}

fn stats_for(app: &AppManifest, by_repo: &BTreeMap<String, &Entry>, month: &str) -> Option<Stats> {
    let e = by_repo.get(&app.repo.as_ref()?.to_ascii_lowercase())?;
    Some(Stats {
        stars: e.stars,
        updated_at: e.updated_at.clone(),
        release: e.release.as_ref().map(|r| r.tag.clone()),
        commits_this_month: e.commits_in(month),
        licenses: e.licenses.clone(),
    })
}

async fn catalog(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let (installed, running) = st.installed().await;
    let snap = st.snapshot().await;
    let settings = st.settings.read().await.clone();
    let policy = settings.capacity.policy();
    let ts = st.ts_host().await;
    let ratings = st.persisted.read().await.ratings.clone();
    let disc = st.discover.read().await;
    let month = hearth_core::discover::latest_month(&disc.entries).unwrap_or_default();
    let by_repo: BTreeMap<String, &Entry> = disc
        .entries
        .iter()
        .filter_map(|e| Some((e.repo.clone()?, e)))
        .collect();
    let builtin_ids: Vec<&str> = st.builtin.iter().map(|a| a.id.as_str()).collect();
    let catalog = st.catalog.read().await;
    let apps: Vec<CatalogEntry> = catalog
        .iter()
        .map(|m| {
            let o = settings.apps.get(&m.id).cloned().unwrap_or_default();
            let port = o.port.unwrap_or(m.port);
            let subdomain = o.subdomain.clone().unwrap_or_else(|| m.id.clone());
            let icon_file = m.icon_file();
            let is_installed = installed.contains(&m.id);
            CatalogEntry {
                id: m.id.clone(),
                name: m.name.clone(),
                description: m.description.clone(),
                category: m.category.clone(),
                website: m.website.clone(),
                repo: m.repo.clone(),
                featured: m.featured,
                editors_note: m.editors_note.clone(),
                icon: st
                    .config
                    .icons_dir()
                    .join(&icon_file)
                    .exists()
                    .then(|| format!("/api/icons/{icon_file}")),
                port,
                url: format!("https://{ts}:{port}"),
                domain_url: settings
                    .domain
                    .as_ref()
                    .filter(|_| o.public)
                    .map(|d| format!("https://{subdomain}.{d}")),
                subdomain,
                public: o.public,
                ram_mb: m.resources.ram_mb,
                needs_vault: m.resources.needs_vault,
                roles: m
                    .roles()
                    .into_iter()
                    .map(hearth_core::storage::Role::as_str)
                    .collect(),
                protected: m.auth.protect,
                custom: !builtin_ids.contains(&m.id.as_str()),
                installed: is_installed,
                running: running.contains(&m.id),
                verdict: (!is_installed)
                    .then_some(snap)
                    .flatten()
                    .map(|s| capacity::evaluate(m, &s, &policy)),
                stats: stats_for(m, &by_repo, &month),
                rating: ratings.get(&m.id).copied(),
            }
        })
        .collect();
    Json(json!({ "apps": apps, "month": month }))
}

async fn manifest(st: &SharedState, id: &str) -> Result<AppManifest, ApiError> {
    st.catalog
        .read()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("app `{id}`")))
}

fn config_path(st: &SharedState, id: &str) -> std::path::PathBuf {
    st.compose.app_dir(id).join("config.json")
}

fn load_config(st: &SharedState, id: &str) -> Option<AppConfig> {
    store::load::<Option<AppConfig>>(&config_path(st, id))
        .ok()
        .flatten()
}

#[derive(Serialize)]
struct ConfigResponse {
    /// Recommended settings straight from the manifest.
    defaults: Vec<ServiceConfig>,
    /// What's saved for this app (if anything).
    saved: Option<AppConfig>,
    /// Manifest compose (for the raw editor).
    compose: String,
    restart_policies: &'static [&'static str],
}

async fn get_config(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<ConfigResponse>, ApiError> {
    let app = manifest(&st, &id).await?;
    Ok(Json(ConfigResponse {
        defaults: compose_config::extract(&app.compose).map_err(bad)?,
        saved: load_config(&st, &id),
        compose: app.compose,
        restart_policies: compose_config::RESTART_POLICIES,
    }))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InstallRequest {
    dry_run: bool,
    force: bool,
    /// Custom Docker settings; omit for the recommended defaults.
    config: Option<AppConfig>,
}

async fn install(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    body: Option<Json<InstallRequest>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let req = body.map(|Json(b)| b).unwrap_or_default();
    let mut app = manifest(&st, &id).await?;
    let config = req.config.clone().or_else(|| load_config(&st, &id));
    app.compose = compose_config::resolve(&app.compose, config.as_ref()).map_err(bad)?;

    let snap = st
        .snapshot()
        .await
        .ok_or_else(|| bad("No host metrics yet; try again in a few seconds."))?;
    let verdict = capacity::evaluate(&app, &snap, &st.settings.read().await.capacity.policy());
    let plan = st.compose.plan(&app);
    if req.dry_run {
        return Ok(Json(
            json!({ "dry_run": true, "verdict": verdict, "plan": plan, "compose": app.compose }),
        ));
    }
    if verdict.fit == Fit::Deny && !req.force {
        return Err(ApiError::Conflict(
            json!({ "error": "does not fit", "verdict": verdict }),
        ));
    }

    let _ops = st.ops.lock().await;
    let (plan, gw) = deploy(&st, &app, config.as_ref()).await.map_err(internal)?;
    Ok(Json(json!({
        "installed": id, "verdict": verdict, "plan": plan,
        "gateway": gw.map_err(|e| format!("{e:#}")),
    })))
}

/// Install (or re-deploy) an app whose compose is already resolved. Caller holds `st.ops`.
pub(crate) async fn deploy(
    st: &SharedState,
    app: &AppManifest,
    config: Option<&AppConfig>,
) -> anyhow::Result<(
    hearth_host::compose::InstallPlan,
    anyhow::Result<crate::gateway::Outcome>,
)> {
    let env = st.compose_env().await;
    let plan = st.compose.install(app, &env).await?;
    if let Some(cfg) = config {
        store::save(&config_path(st, &app.id), &Some(cfg))?;
    }
    let gw = gateway::apply(st).await;
    st.event(
        Level::Success,
        "apps",
        format!("{} installed", app.name),
        "It's running.",
    )
    .await;
    Ok((plan, gw))
}

/// Resolve an app's compose with its saved config, ready for [`deploy`].
pub(crate) async fn prepared(
    st: &SharedState,
    id: &str,
) -> Result<(AppManifest, Option<AppConfig>), ApiError> {
    let mut app = manifest(st, id).await?;
    let config = load_config(st, id);
    app.compose = compose_config::resolve(&app.compose, config.as_ref()).map_err(bad)?;
    Ok((app, config))
}

async fn remove(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app = manifest(&st, &id).await?;
    let _ops = st.ops.lock().await;
    st.compose
        .remove(&app, &st.compose_env().await)
        .await
        .map_err(internal)?;
    gateway::reapply_quietly(&st).await;
    st.event(
        Level::Info,
        "apps",
        format!("{} removed", app.name),
        "Data kept on disk.",
    )
    .await;
    Ok(Json(
        json!({ "removed": id, "data_kept_in": st.compose.app_dir(&id) }),
    ))
}

async fn lifecycle(
    State(st): State<SharedState>,
    Path((id, action)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app = manifest(&st, &id).await?;
    let env = st.compose_env().await;
    let r = match action.as_str() {
        "start" => st.compose.start(&app, &env).await,
        "stop" => st.compose.stop(&app, &env).await,
        "restart" => st.compose.restart(&app, &env).await,
        _ => return Err(ApiError::NotFound(format!("action `{action}`"))),
    };
    r.map_err(internal)?;
    st.event(Level::Info, "apps", format!("{} {action}ed", app.name), "")
        .await;
    Ok(Json(json!({ "ok": true })))
}

async fn put_override(
    State(st): State<SharedState>,
    Path(id): Path<String>,
    Json(o): Json<AppOverride>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app = manifest(&st, &id).await?;
    if let Some(p) = o.port {
        st.port_free(p, &id).await.map_err(bad)?;
    }
    {
        let mut s = st.settings.write().await;
        let mut next = s.clone();
        if o == AppOverride::default() {
            next.apps.remove(&id);
        } else {
            next.apps.insert(id.clone(), o);
        }
        next.validate().map_err(bad)?;
        store::save(&st.config.state_dir.join("settings.json"), &next).map_err(internal)?;
        *s = next;
    }
    gateway::reapply_quietly(&st).await;
    st.event(
        Level::Success,
        "apps",
        format!("{} updated", app.name),
        "Address settings saved.",
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

/// Bring-your-own app: either a compose file, or a simple image form.
#[derive(Debug, Deserialize)]
struct CustomApp {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_category")]
    category: String,
    port: u16,
    /// Compose service + container port the gateway forwards to.
    service: Option<String>,
    container_port: u16,
    #[serde(default)]
    ram_mb: u64,
    /// Full compose YAML (advanced).
    compose: Option<String>,
    /// Simple mode: image + env + volumes.
    image: Option<String>,
    #[serde(default)]
    env: Vec<(String, String)>,
    #[serde(default)]
    volumes: Vec<String>,
    #[serde(default = "yes")]
    protect: bool,
}

fn default_category() -> String {
    "custom".into()
}
const fn yes() -> bool {
    true
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    s.split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(40)
        .collect()
}

async fn create_custom(
    State(st): State<SharedState>,
    Json(c): Json<CustomApp>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = slug(&c.name);
    if id.is_empty() {
        return Err(bad("Give the app a name."));
    }
    if st.catalog.read().await.get(&id).is_some() {
        return Err(bad(format!("An app with id `{id}` already exists.")));
    }
    st.port_free(c.port, &id).await.map_err(bad)?;
    let (compose, service) = match (&c.compose, &c.image) {
        (Some(y), _) if !y.trim().is_empty() => {
            let svc = c
                .service
                .clone()
                .ok_or_else(|| bad("Pick which service the gateway should open."))?;
            (y.clone(), svc)
        }
        (_, Some(image)) if !image.trim().is_empty() => {
            let mut svc = serde_yaml_ng::Mapping::new();
            svc.insert("image".into(), image.trim().into());
            svc.insert("restart".into(), "unless-stopped".into());
            if !c.env.is_empty() {
                let env: serde_yaml_ng::Mapping = c
                    .env
                    .iter()
                    .map(|(k, v)| (k.as_str().into(), v.as_str().into()))
                    .collect();
                svc.insert("environment".into(), env.into());
            }
            if !c.volumes.is_empty() {
                svc.insert(
                    "volumes".into(),
                    c.volumes
                        .iter()
                        .map(|v| serde_yaml_ng::Value::from(v.as_str()))
                        .collect::<Vec<_>>()
                        .into(),
                );
            }
            let mut services = serde_yaml_ng::Mapping::new();
            services.insert("app".into(), svc.into());
            let mut doc = serde_yaml_ng::Mapping::new();
            doc.insert("services".into(), services.into());
            (
                serde_yaml_ng::to_string(&doc).map_err(internal)?,
                "app".to_owned(),
            )
        }
        _ => return Err(bad("Provide either an image or a compose file.")),
    };
    let manifest = serde_yaml_ng::to_string(&json!({
        "id": id, "name": c.name.trim(), "description": if c.description.is_empty() { "Custom app".into() } else { c.description.clone() },
        "category": c.category, "port": c.port,
        "upstream": { "service": service, "port": c.container_port },
        "resources": { "ram_mb": c.ram_mb, "needs_vault": compose.contains("${VAULT}") },
        "auth": { "protect": c.protect },
        "compose": compose,
    }))
    .map_err(internal)?;
    AppManifest::from_yaml(&manifest).map_err(bad)?;
    let path = st.config.custom_dir().join(format!("{id}.yml"));
    std::fs::create_dir_all(st.config.custom_dir()).map_err(internal)?;
    std::fs::write(&path, manifest).map_err(internal)?;
    st.reload_catalog().await.map_err(|e| {
        let _ = std::fs::remove_file(&path);
        bad(format!("{e:#}"))
    })?;
    st.event(
        Level::Success,
        "apps",
        format!("{} added", c.name),
        "Find it under Apps → Custom. Install it from there.",
    )
    .await;
    Ok(Json(json!({ "id": id })))
}

async fn delete_custom(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if st.builtin.iter().any(|a| a.id == id) {
        return Err(bad("Built-in apps can't be deleted."));
    }
    if st.installed().await.0.contains(&id) {
        return Err(bad("Remove the app first."));
    }
    let path = st.config.custom_dir().join(format!("{id}.yml"));
    std::fs::remove_file(&path).map_err(|_| ApiError::NotFound(format!("custom app `{id}`")))?;
    st.reload_catalog().await.map_err(internal)?;
    Ok(Json(json!({ "deleted": id })))
}

#[derive(Deserialize)]
struct Rating {
    stars: Option<u8>,
}

async fn put_rating(
    State(st): State<SharedState>,
    Path(key): Path<String>,
    Json(r): Json<Rating>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if key.is_empty()
        || key.len() > 80
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(bad("Invalid app key."));
    }
    let mut p = st.persisted.write().await;
    match r.stars {
        Some(s @ 1..=5) => {
            p.ratings.insert(key, s);
        }
        None => {
            p.ratings.remove(&key);
        }
        Some(_) => return Err(bad("Ratings are 1-5 stars.")),
    }
    st.save_persisted(&p).map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

/// Curated logos (`catalog/icons/*.svg`) and directory logos (`<slug>.webp`).
/// Extensions are matched exactly on purpose: we only ever generate lowercase names.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
async fn icon(
    State(st): State<SharedState>,
    Path(file): Path<String>,
) -> Result<Response, ApiError> {
    let ok = file.len() <= 100
        && file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !file.starts_with('.');
    if !ok {
        return Err(ApiError::NotFound("icon".into()));
    }
    let (path, mime) = if let Some(slug) = file.strip_suffix(".webp") {
        (
            crate::discover::data_dir(&st.config.state_dir)
                .join("icons")
                .join(format!("{slug}.webp")),
            "image/webp",
        )
    } else if file.ends_with(".svg") {
        (st.config.icons_dir().join(&file), "image/svg+xml")
    } else if file.ends_with(".png") {
        (st.config.icons_dir().join(&file), "image/png")
    } else {
        return Err(ApiError::NotFound("icon".into()));
    };
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::NotFound("icon".into()))?;
    let mut res = (StatusCode::OK, Body::from(bytes)).into_response();
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    // SVGs are images, never documents: block scripts if opened directly.
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'; sandbox"),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(res)
}

async fn history(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let app = manifest(&st, &id).await?;
    let h = st.history.read().await;
    Ok(Json(json!({
        "summary": h.summary(&app.project_name()),
        "estimate_mb": app.resources.ram_mb,
        "measured_mb": h.measured_ram_mb(&app.project_name()),
    })))
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugs() {
        assert_eq!(slug("My Cool App!"), "my-cool-app");
        assert_eq!(slug("  --  "), "");
    }
}
