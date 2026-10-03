//! Shared API route helpers.
use crate::api::response::ErrorResponse;
use axum::{Json, http::StatusCode, response::IntoResponse};

pub(super) async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse::failure(
            crate::api::api_error_code::ApiErrorCode::EndpointNotFound,
        )),
    )
}
