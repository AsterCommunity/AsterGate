//! Product response cache/privacy policy, including early HTTP rejections.
use axum::{extract::Request, http::header, middleware::Next, response::Response};

/// Outer response policy also covers CORS/limiter rejections before the account boundary.
pub async fn apply(request: Request, next: Next) -> Response {
    let protected = request.uri().path().starts_with("/api/v1/");
    let mut response = next.run(request).await;
    if !protected {
        return response;
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::PRAGMA,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    response
}
