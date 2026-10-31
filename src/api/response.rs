//! Product response policy over Forge's controlled REST envelope.
//! Serialization, field omission and success/failure state belong to Forge; Gate owns codes,
//! messages, retryability and diagnostic disclosure. Protocol and health responses stay separate.

use super::api_error_code::ApiErrorCode;
use aster_forge_api::response::{ApiErrorInfo, ApiResponse, ResponseEnvelopeError};
use serde::Serialize;

/// Named unit alias accepted by the OpenAPI attribute parser; no wire wrapper.
pub type NoData = ();

/// Liveness/readiness wire contract, deliberately outside the product REST envelope.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct StatusResponse {
    pub service: &'static str,
    pub status: &'static str,
}

/// Inject Gate's success code; construction failures are internal security failures.
pub fn success<T>(data: T) -> crate::services::account::Result<ApiResponse<T, ApiErrorCode>> {
    ApiResponse::ok(ApiErrorCode::Success, data)
        .map_err(|_| crate::services::account::AccountError::Security)
}

/// Public failures disclose only Gate's stable message and retryability.
/// No internal diagnostic or error source is accepted by this public boundary.
pub fn failure(code: ApiErrorCode) -> Result<ApiResponse<(), ApiErrorCode>, ResponseEnvelopeError> {
    ApiResponse::error_with_details(
        code,
        code.message(),
        Some(ApiErrorInfo {
            retryable: matches!(
                code,
                ApiErrorCode::RateLimited | ApiErrorCode::ServiceUnavailable
            ),
            diagnostic: None,
        }),
    )
}
