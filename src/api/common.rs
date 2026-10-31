//! Shared API route helpers.
use axum::response::IntoResponse;

pub(super) async fn api_not_found() -> impl IntoResponse {
    crate::api::api_error_code::ApiErrorCode::EndpointNotFound.into_response()
}
