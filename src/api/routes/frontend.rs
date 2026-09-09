//! Frontend asset routes.
use aster_forge_utils::html::escape_html;
use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderValue, Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;
use std::{path::PathBuf, sync::Arc};

#[derive(Embed)]
#[folder = "$ASTER_FRONTEND_DIST_DIR"]
struct FrontendAssets;
const CUSTOM_FRONTEND_DIR: &str = "./frontend-override";
const FILE_NOT_FOUND_MESSAGE: &str = "File not found";
const INDEX_CACHE_CONTROL: &str = "no-cache";
const IMMUTABLE_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const STATIC_ASSET_CACHE_CONTROL: &str = "public, max-age=86400";
const PWA_CACHE_CONTROL: &str = "no-cache";
const FRONTEND_TITLE: &str = "aster_gate";
const FRONTEND_DESCRIPTION: &str = "Aster service wired to AsterForge runtime components.";
const FRONTEND_FAVICON_URL: &str = "/favicon.svg";
pub const FRONTEND_CSP_HEADER: &str = "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' http: https: ws: wss:; worker-src 'self' blob:; manifest-src 'self'";
pub const FRONTEND_CSP_META: &str = "default-src 'self'; base-uri 'self'; object-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' http: https: ws: wss:; worker-src 'self' blob:; manifest-src 'self'";

pub struct FrontendService;
impl FrontendService {
    async fn load_file(path: &str) -> Option<Vec<u8>> {
        if path.contains("..") {
            return None;
        }
        if let Ok(data) = tokio::fs::read(PathBuf::from(CUSTOM_FRONTEND_DIR).join(path)).await {
            return Some(data);
        }
        FrontendAssets::get(path).map(|file| file.data.into_owned())
    }
    fn content_type(path: &str) -> &'static str {
        match path.rsplit('.').next() {
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
    fn process_index(html: &str) -> String {
        html.replace("%ASTER_SERVICE_VERSION%", env!("CARGO_PKG_VERSION"))
            .replace("%ASTER_SERVICE_TITLE%", &escape_html(FRONTEND_TITLE))
            .replace(
                "%ASTER_SERVICE_DESCRIPTION%",
                &escape_html(FRONTEND_DESCRIPTION),
            )
            .replace(
                "%ASTER_SERVICE_FAVICON_URL%",
                &escape_html(FRONTEND_FAVICON_URL),
            )
            .replace("%ASTER_SERVICE_CSP%", &escape_html(FRONTEND_CSP_META))
    }
    fn manifest(value: &str) -> String {
        value
            .replace("%ASTER_SERVICE_TITLE%", FRONTEND_TITLE)
            .replace("%ASTER_SERVICE_DESCRIPTION%", FRONTEND_DESCRIPTION)
    }
    async fn index_response() -> Response {
        let html = Self::load_file("index.html").await.unwrap_or_else(|| {
            include_str!(concat!(env!("ASTER_FRONTEND_DIST_DIR"), "/index.html"))
                .as_bytes()
                .to_vec()
        });
        let mut response = Self::response(
            StatusCode::OK,
            "text/html; charset=utf-8",
            INDEX_CACHE_CONTROL,
            Self::process_index(&String::from_utf8_lossy(&html)).into_bytes(),
        );
        response.headers_mut().insert(
            "content-security-policy",
            HeaderValue::from_static(FRONTEND_CSP_HEADER),
        );
        response
    }
    fn response(status: StatusCode, content_type: &str, cache: &str, body: Vec<u8>) -> Response {
        (
            status,
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, cache),
            ],
            body,
        )
            .into_response()
    }
    pub async fn index() -> Response {
        Self::index_response().await
    }
    pub async fn asset(Path(path): Path<String>) -> Response {
        let ty = Self::content_type(&path);
        Self::load_file(&format!("assets/{path}"))
            .await
            .map_or_else(
                || (StatusCode::NOT_FOUND, FILE_NOT_FOUND_MESSAGE).into_response(),
                |data| Self::response(StatusCode::OK, ty, IMMUTABLE_ASSET_CACHE_CONTROL, data),
            )
    }
    pub async fn static_asset(Path(path): Path<String>) -> Response {
        let ty = Self::content_type(&path);
        Self::load_file(&format!("static/{path}"))
            .await
            .map_or_else(
                || (StatusCode::NOT_FOUND, FILE_NOT_FOUND_MESSAGE).into_response(),
                |data| Self::response(StatusCode::OK, ty, STATIC_ASSET_CACHE_CONTROL, data),
            )
    }
    pub async fn favicon() -> Response {
        Self::load_file("favicon.svg").await.map_or_else(
            || {
                Self::response(
                    StatusCode::OK,
                    "image/svg+xml",
                    STATIC_ASSET_CACHE_CONTROL,
                    Vec::new(),
                )
            },
            |data| {
                Self::response(
                    StatusCode::OK,
                    "image/svg+xml",
                    STATIC_ASSET_CACHE_CONTROL,
                    data,
                )
            },
        )
    }
    pub async fn pwa(request: Request<Body>) -> Response {
        let filename = request.uri().path().trim_start_matches('/');
        let Some(data) = Self::load_file(filename).await else {
            return (StatusCode::NOT_FOUND, FILE_NOT_FOUND_MESSAGE).into_response();
        };
        let body = if filename == "manifest.webmanifest" {
            Self::manifest(&String::from_utf8_lossy(&data)).into_bytes()
        } else {
            data
        };
        Self::response(
            StatusCode::OK,
            Self::content_type(filename),
            PWA_CACHE_CONTROL,
            body,
        )
    }
    pub async fn fallback(State(_state): State<Arc<crate::runtime::AppState>>) -> Response {
        Self::index_response().await
    }
}

pub fn router() -> Router<Arc<crate::runtime::AppState>> {
    Router::new()
        .route("/", get(FrontendService::index))
        .route("/index.html", get(FrontendService::index))
        .route("/assets/{*path}", get(FrontendService::asset))
        .route("/static/{*path}", get(FrontendService::static_asset))
        .route("/favicon.svg", get(FrontendService::favicon))
        .route("/registerSW.js", get(FrontendService::pwa))
        .route("/manifest.webmanifest", get(FrontendService::pwa))
        .route("/sw.js", get(FrontendService::pwa))
        .route("/{filename}", get(FrontendService::pwa))
        .fallback(FrontendService::fallback)
}
