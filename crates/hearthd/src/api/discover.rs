//! Discover: the awesome-selfhosted directory with search and leaderboards.

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use hearth_core::discover::{self, ATTRIBUTION};
use serde_json::json;

use super::bad;
use crate::error::ApiError;
use crate::state::SharedState;

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/discover", get(search))
        .route("/discover/meta", get(meta))
        .route("/discover/leaderboards", get(leaderboards))
        .route("/discover/sync", post(sync))
        .route("/discover/app/{slug}", get(detail))
        .route("/discover/app/{slug}/preview", get(preview))
}

async fn search(
    State(st): State<SharedState>,
    Query(mut q): Query<discover::Query>,
) -> Json<serde_json::Value> {
    if q.per_page == 0 {
        q.per_page = 48;
    }
    if !q.q.trim().is_empty() && st.search.read().await.is_none() {
        crate::search::rebuild(&st).await;
    }
    let d = st.discover.read().await;
    let ratings = st.persisted.read().await.ratings.clone();
    // With a text query, rank by full-text relevance (BM25 + typo tolerance), then apply filters.
    let ranked: Option<Vec<String>> = if q.q.trim().is_empty() {
        None
    } else {
        st.search
            .read()
            .await
            .as_ref()
            .and_then(|ix| ix.search(&q.q, &["discover"], 1000).ok())
            .map(|hits| hits.into_iter().map(|h| h.doc.id).collect())
    };
    let page = match &ranked {
        Some(slugs) => {
            let by_slug: std::collections::HashMap<&str, &hearth_core::discover::Entry> =
                d.entries.iter().map(|e| (e.slug.as_str(), e)).collect();
            let filtered: Vec<hearth_core::discover::Entry> = slugs
                .iter()
                .filter_map(|s| by_slug.get(s.as_str()).copied())
                .filter(|e| !q.docker_only || e.docker())
                .filter(|e| q.tag.as_ref().is_none_or(|t| e.tags.iter().any(|x| x == t)))
                .cloned()
                .collect();
            let mut unq = q.clone();
            unq.q = String::new();
            let total = filtered.len();
            let per = unq.per_page.clamp(1, 100);
            let items: Vec<_> = filtered
                .into_iter()
                .skip(unq.page * per)
                .take(per)
                .collect();
            let items: Vec<_> = items
                .iter()
                .map(|e| json!({ "entry": e, "rating": ratings.get(&e.slug) }))
                .collect();
            return Json(
                json!({ "total": total, "page": unq.page, "per_page": per, "items": items, "ranked": true }),
            );
        }
        None => discover::search(&d.entries, &q),
    };
    let items: Vec<_> = page
        .items
        .iter()
        .map(|e| json!({ "entry": e, "rating": ratings.get(&e.slug) }))
        .collect();
    Json(
        json!({ "total": page.total, "page": page.page, "per_page": page.per_page, "items": items }),
    )
}

async fn meta(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let d = st.discover.read().await;
    Json(json!({
        "total": d.entries.len(),
        "tags": discover::tag_counts(&d.entries),
        "synced_at": d.synced_at,
        "syncing": d.syncing,
        "error": d.error,
        "month": discover::latest_month(&d.entries),
        "attribution": ATTRIBUTION,
        "source": "https://awesome-selfhosted.net",
    }))
}

async fn leaderboards(State(st): State<SharedState>) -> Json<serde_json::Value> {
    let d = st.discover.read().await;
    Json(json!(discover::leaderboards(&d.entries, 10)))
}

async fn sync(State(st): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let n = crate::discover::sync(&st)
        .await
        .map_err(|e| bad(format!("{e:#}")))?;
    Ok(Json(json!({ "apps": n })))
}

fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !s.starts_with('.')
}

/// One directory entry + whether it's installable from the curated catalog.
async fn detail(
    State(st): State<SharedState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !valid_slug(&slug) {
        return Err(ApiError::NotFound("app".into()));
    }
    let d = st.discover.read().await;
    let e = d
        .entries
        .iter()
        .find(|e| e.slug == slug)
        .cloned()
        .ok_or_else(|| ApiError::NotFound(format!("app `{slug}`")))?;
    drop(d);
    let catalog_id = match &e.repo {
        Some(r) => st
            .catalog
            .read()
            .await
            .iter()
            .find(|a| a.repo.as_ref().is_some_and(|x| x.eq_ignore_ascii_case(r)))
            .map(|a| a.id.clone()),
        None => None,
    };
    let rating = st.persisted.read().await.ratings.get(&slug).copied();
    let related: Vec<serde_json::Value> = {
        let d = st.discover.read().await;
        let tag = e.tags.first().cloned();
        let mut rel: Vec<&hearth_core::discover::Entry> = d
            .entries
            .iter()
            .filter(|x| {
                x.slug != e.slug && !x.archived && tag.as_ref().is_some_and(|t| x.tags.contains(t))
            })
            .collect();
        rel.sort_by_key(|x| std::cmp::Reverse(x.stars));
        rel.iter()
            .take(6)
            .map(|x| json!({ "slug": x.slug, "name": x.name, "stars": x.stars, "has_icon": x.has_icon, "description": x.description }))
            .collect()
    };
    Ok(Json(json!({
        "entry": e, "rating": rating, "catalog_id": catalog_id, "related": related,
        "has_preview": e.repo.is_some(),
        "directory_url": format!("https://awesome-selfhosted.net/tags/{}.html", e.tags.first().map(|t| t.to_lowercase().replace(' ', "-").replace(['(', ')', '&'], "")).unwrap_or_default()),
    })))
}

/// GitHub's social preview card for the repo, fetched server-side and cached on disk.
async fn preview(
    State(st): State<SharedState>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> Result<axum::response::Response, ApiError> {
    use axum::http::header;
    use axum::response::IntoResponse as _;
    if !valid_slug(&slug) {
        return Err(ApiError::NotFound("preview".into()));
    }
    let repo = st
        .discover
        .read()
        .await
        .entries
        .iter()
        .find(|e| e.slug == slug)
        .and_then(|e| e.repo.clone())
        .ok_or_else(|| ApiError::NotFound("preview".into()))?;
    let cache = st
        .config
        .state_dir
        .join("discover/previews")
        .join(format!("{slug}.png"));
    let bytes = if let Ok(b) = tokio::fs::read(&cache).await {
        b
    } else {
        let url = format!("https://opengraph.githubassets.com/1/{repo}");
        let res = reqwest::Client::builder()
            .user_agent("hearth")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(bad)?
            .get(url)
            .send()
            .await
            .map_err(|_| ApiError::NotFound("preview".into()))?;
        if !res.status().is_success() {
            return Err(ApiError::NotFound("preview".into()));
        }
        let b = res
            .bytes()
            .await
            .map_err(|_| ApiError::NotFound("preview".into()))?
            .to_vec();
        if let Some(dir) = cache.parent() {
            let _ = tokio::fs::create_dir_all(dir).await;
        }
        let _ = tokio::fs::write(&cache, &b).await;
        b
    };
    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=604800"),
        ],
        bytes,
    )
        .into_response())
}
