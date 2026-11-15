//! Serves the React dashboard (`web/apps/dashboard/dist`) embedded into the binary.
//!
//! Debug builds read the folder from disk at runtime (rebuild the UI, refresh the browser).
//! Release builds embed it. Unknown non-asset paths fall back to `index.html` (SPA routing).

use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/apps/dashboard/dist"]
#[allow_missing = true]
struct Assets;

const NOT_BUILT: &str = "<!doctype html><meta charset=utf-8><title>hearth</title>\
<body style=\"font:14px system-ui;padding:40px\"><h1>Dashboard not built</h1>\
<p>Run <code>pnpm -C web install &amp;&amp; pnpm -C web build</code>, then rebuild hearthd.</p>\
<p>The API is up at <a href=\"/api/health\">/api/health</a>.</p>";

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = Assets::get(path) {
        return asset(path, &file);
    }
    // Missing hashed asset → real 404; anything else is a client-side route.
    if path.starts_with("assets/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match Assets::get("index.html") {
        Some(index) => asset("index.html", &index),
        None => Html(NOT_BUILT).into_response(),
    }
}

fn asset(path: &str, file: &rust_embed::EmbeddedFile) -> Response {
    let mime = file.metadata.mimetype();
    // Vite fingerprints everything under assets/, so it can be cached forever.
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut res = file.data.clone().into_response();
    let headers = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(mime) {
        headers.insert(header::CONTENT_TYPE, v);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    res
}
