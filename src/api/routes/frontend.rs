//! Embedded product frontend. SPA navigation and static assets have distinct response contracts.
use crate::{
    runtime::AppState,
    services::account::{AccountError, Result},
};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;
use std::sync::Arc;

#[derive(Embed)]
#[folder = "$ASTER_FRONTEND_DIST_DIR"]
struct FrontendAssets;

const INDEX_CACHE_CONTROL: &str = "no-cache";
const IMMUTABLE_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const STATIC_ASSET_CACHE_CONTROL: &str = "public, max-age=86400";
const PWA_CACHE_CONTROL: &str = "no-cache";
pub const FRONTEND_CSP_HEADER: &str = "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' http: https: ws: wss:; worker-src 'self' blob:; manifest-src 'self'";

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js" | "mjs") => "application/javascript",
        Some("json") => "application/json",
        Some("webmanifest") => "application/manifest+json",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn file_response(path: &str, cache: &'static str) -> Response {
    let Some(file) = FrontendAssets::get(path) else {
        return (StatusCode::NOT_FOUND, "File not found").into_response();
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type(path)),
            (header::CACHE_CONTROL, cache),
        ],
        file.data,
    )
        .into_response()
}

pub async fn index(State(state): State<Arc<AppState>>) -> Result<Response> {
    let branding = state.runtime_config.branding()?;
    let file = FrontendAssets::get("index.html").ok_or(AccountError::Storage)?;
    let html = std::str::from_utf8(&file.data).map_err(|_| AccountError::Storage)?;
    let html = aster_forge_mail::render_placeholders(
        html,
        &[
            (
                "astergate_title",
                aster_forge_utils::html::escape_html(branding.title.as_str()),
            ),
            (
                "astergate_description",
                aster_forge_utils::html::escape_html(branding.description.as_str()),
            ),
        ],
    );
    let mut response = (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, INDEX_CACHE_CONTROL),
        ],
        html,
    )
        .into_response();
    response.headers_mut().insert(
        "content-security-policy",
        HeaderValue::from_static(FRONTEND_CSP_HEADER),
    );
    Ok(response)
}

pub async fn manifest(State(state): State<Arc<AppState>>) -> Result<Response> {
    let branding = state.runtime_config.branding()?;
    let value = serde_json::json!({
        "name": branding.title.as_str(), "short_name": branding.title.as_str(),
        "description": branding.description.as_str(), "id": "/", "start_url": "/", "scope": "/",
        "display": "standalone", "theme_color": "#19654e", "background_color": "#ffffff",
        "icons": [{"src":"/favicon.svg","sizes":"any","type":"image/svg+xml","purpose":"any"}]
    });
    Ok((
        [
            (header::CONTENT_TYPE, "application/manifest+json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        axum::Json(value),
    )
        .into_response())
}

pub async fn asset(Path(path): Path<String>) -> Response {
    file_response(&format!("assets/{path}"), IMMUTABLE_ASSET_CACHE_CONTROL)
}

pub async fn static_asset(Path(path): Path<String>) -> Response {
    file_response(&format!("static/{path}"), STATIC_ASSET_CACHE_CONTROL)
}

pub async fn favicon() -> Response {
    file_response("favicon.svg", STATIC_ASSET_CACHE_CONTROL)
}

pub async fn pwa(uri: Uri) -> Response {
    file_response(uri.path().trim_start_matches('/'), PWA_CACHE_CONTROL)
}

pub async fn navigation(State(state): State<Arc<AppState>>, uri: Uri) -> Result<Response> {
    let path = uri.path().trim_start_matches('/');
    if !path.contains('/') && path.starts_with("workbox-") && path.ends_with(".js") {
        return Ok(file_response(path, PWA_CACHE_CONTROL));
    }
    if path
        .rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
    {
        return Ok((StatusCode::NOT_FOUND, "File not found").into_response());
    }
    index(State(state)).await
}

pub fn router() -> Router<Arc<crate::runtime::AppState>> {
    Router::new()
        .route("/", get(index))
        .route("/index.html", get(index))
        .route("/assets/{*path}", get(asset))
        .route("/static/{*path}", get(static_asset))
        .route("/favicon.svg", get(favicon))
        .route("/registerSW.js", get(pwa))
        .route("/manifest.webmanifest", get(manifest))
        .route("/sw.js", get(pwa))
        .fallback(get(navigation))
}
