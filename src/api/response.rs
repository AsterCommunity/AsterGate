//! API response models.

use serde::Serialize;

/// Basic status response returned by the generated skeleton.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct StatusResponse {
    /// Cargo package name.
    pub service: &'static str,
    /// Public health or readiness status.
    pub status: &'static str,
}

/// Product error envelope, matching the AsterDrive wire contract.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct ErrorResponse {
    pub code: super::api_error_code::ApiErrorCode,
    pub msg: String,
    pub error: ApiErrorInfo,
}
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
pub struct ApiErrorInfo {
    pub retryable: bool,
}

impl ErrorResponse {
    pub fn failure(code: super::api_error_code::ApiErrorCode) -> Self {
        Self {
            code,
            msg: code.message().into(),
            error: ApiErrorInfo {
                retryable: matches!(
                    code,
                    super::api_error_code::ApiErrorCode::RateLimited
                        | super::api_error_code::ApiErrorCode::ServiceUnavailable
                ),
            },
        }
    }
}

// Concrete wire wrappers prevent generic aliases from colliding in OpenAPI component names.
macro_rules! response {
    ($name:ident, $data:ty) => {
        #[derive(serde::Serialize)]
        #[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
        pub struct $name {
            pub code: crate::api::api_error_code::ApiErrorCode,
            pub msg: String,
            pub data: $data,
        }
        impl $name {
            pub fn ok(data: $data) -> Self {
                Self {
                    code: crate::api::api_error_code::ApiErrorCode::Success,
                    msg: String::new(),
                    data,
                }
            }
        }
    };
}
pub(crate) use response;
