//! Gate consumes Forge's controlled wire state without introducing a second serializer.
#[path = "common/mod.rs"]
mod common;

use aster_forge_api::response::{ApiEmptyData, ApiErrorInfo, ApiResponse};
use aster_gate::api::{api_error_code::ApiErrorCode, response};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::Path,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use serde_json::{Value, json};
use tower::ServiceExt;

#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
struct Profile {
    id: u64,
    name: &'static str,
}
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
struct Task {
    completed: bool,
}
#[derive(Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(utoipa::ToSchema))]
struct Diagnostic {
    kind: &'static str,
}

async fn envelope(Path(case): Path<String>) -> Response {
    match case.as_str() {
        "profile" => Json(
            response::success(Profile {
                id: 7,
                name: "猫猫",
            })
            .unwrap(),
        )
        .into_response(),
        "task" => Json(response::success(Task { completed: true }).unwrap()).into_response(),
        "absent" => Json(ApiResponse::<(), ApiErrorCode>::ok_empty(ApiErrorCode::Success).unwrap())
            .into_response(),
        "null" => Json(response::success(()).unwrap()).into_response(),
        "empty" => Json(
            ApiResponse::<ApiEmptyData, ApiErrorCode>::ok_empty_data(ApiErrorCode::Success)
                .unwrap(),
        )
        .into_response(),
        "diagnostic" => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(
                ApiResponse::<(), ApiErrorCode, Diagnostic>::error_with_details(
                    ApiErrorCode::RateLimited,
                    "Try later",
                    Some(ApiErrorInfo {
                        retryable: true,
                        diagnostic: Some(Diagnostic { kind: "test_only" }),
                    }),
                )
                .unwrap(),
            ),
        )
            .into_response(),
        "metadata-absent" => (
            StatusCode::BAD_REQUEST,
            Json(
                ApiResponse::<(), ApiErrorCode>::error_with_details(
                    ApiErrorCode::ValidationFailed,
                    "Invalid input",
                    None,
                )
                .unwrap(),
            ),
        )
            .into_response(),
        "unauthenticated" => {
            aster_gate::services::account::AccountError::Unauthenticated.into_response()
        }
        "credentials" => {
            aster_gate::services::account::AccountError::InvalidCredentials.into_response()
        }
        "conflict" => aster_gate::services::account::AccountError::SettingsConflict.into_response(),
        "forbidden" => aster_gate::services::account::AccountError::Forbidden.into_response(),
        "storage" => aster_gate::services::account::AccountError::Storage.into_response(),
        "rate-limit" => aster_gate::services::account::AccountError::RateLimited.into_response(),
        _ => ApiErrorCode::EndpointNotFound.into_response(),
    }
}
async fn body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
async fn request(app: &Router, path: &str) -> Response {
    app.clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn real_router_preserves_multiple_dtos_absence_null_empty_and_diagnostic() {
    let app = Router::new().route("/{case}", get(envelope));
    for (case, expected) in [
        (
            "profile",
            json!({"code":"success","msg":"","data":{"id":7,"name":"猫猫"}}),
        ),
        (
            "task",
            json!({"code":"success","msg":"","data":{"completed":true}}),
        ),
        ("absent", json!({"code":"success","msg":""})),
        ("null", json!({"code":"success","msg":"","data":null})),
        ("empty", json!({"code":"success","msg":"","data":{}})),
        (
            "metadata-absent",
            json!({"code":"validation.failed","msg":"Invalid input"}),
        ),
        (
            "diagnostic",
            json!({"code":"rate_limited","msg":"Try later","error":{"retryable":true,"diagnostic":{"kind":"test_only"}}}),
        ),
    ] {
        let response = request(&app, &format!("/{case}")).await;
        assert_eq!(response.headers()["content-type"], "application/json");
        if case == "diagnostic" {
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        }
        assert_eq!(body(response).await, expected, "{case}");
    }
}

#[test]
fn controlled_states_and_product_retryability_are_preserved() {
    assert!(ApiResponse::<(), ApiErrorCode>::ok(ApiErrorCode::RateLimited, ()).is_err());
    assert!(response::failure(ApiErrorCode::Success).is_err());
    for invalid in [
        json!({"code":"success","msg":"","error":{"retryable":false}}),
        json!({"code":"rate_limited","msg":"","data":null}),
        json!({"code":"unknown","msg":""}),
    ] {
        assert!(serde_json::from_value::<ApiResponse<Value, ApiErrorCode>>(invalid).is_err());
    }
    for &code in ApiErrorCode::ALL {
        if code == ApiErrorCode::Success {
            continue;
        }
        let value = serde_json::to_value(response::failure(code).unwrap()).unwrap();
        assert_eq!(value["code"], code.as_str());
        assert_eq!(value["msg"], code.message());
        assert_eq!(
            value["error"]["retryable"],
            matches!(
                code,
                ApiErrorCode::RateLimited | ApiErrorCode::ServiceUnavailable
            )
        );
        assert!(value.get("data").is_none());
        assert!(value["error"].get("diagnostic").is_none());
    }
}

#[tokio::test]
async fn gate_adapters_keep_http_status_retry_headers_and_hide_internal_errors() {
    let app = Router::new().route("/{case}", get(envelope));
    for (case, code) in [
        ("unauthenticated", ApiErrorCode::SessionRequired),
        ("credentials", ApiErrorCode::CredentialsFailed),
        ("conflict", ApiErrorCode::SettingsConflict),
        ("forbidden", ApiErrorCode::Forbidden),
        ("storage", ApiErrorCode::ServiceUnavailable),
        ("rate-limit", ApiErrorCode::RateLimited),
    ] {
        let response = request(&app, &format!("/{case}")).await;
        assert_eq!(response.status(), code.status());
        if code == ApiErrorCode::RateLimited {
            assert!(response.headers().contains_key("retry-after"));
        }
        let value = body(response).await;
        assert_eq!(value["code"], code.as_str());
        assert!(value.get("data").is_none());
        assert!(value["error"].get("diagnostic").is_none());
    }
}

#[tokio::test]
async fn actual_gate_router_retains_product_envelope_and_protocol_separation() {
    let (state, _database) = common::setup().await;
    let app = aster_gate::api::router(std::sync::Arc::new(state));
    for path in [
        "/api/v1/setup/status",
        "/api/v1/public/frontend-config",
        "/api/v1/account",
    ] {
        let response = request(&app, path).await;
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["referrer-policy"], "no-referrer");
        let status = response.status();
        let value = body(response).await;
        assert_eq!(
            status,
            ApiErrorCode::parse(value["code"].as_str().unwrap())
                .unwrap()
                .status()
        );
        assert!(value["msg"].is_string());
        if value["code"] == "success" {
            assert!(value.get("data").is_some());
            assert!(value.get("error").is_none());
        } else {
            assert!(value.get("data").is_none());
            assert!(value["error"].get("diagnostic").is_none());
        }
    }
    let value = body(request(&app, "/health").await).await;
    assert!(value.get("code").is_none());
    assert!(value["status"].is_string());
}

#[cfg(all(debug_assertions, feature = "openapi"))]
#[test]
fn generic_empty_data_and_diagnostic_schemas_remain_distinct() {
    use aster_gate::api::response::NoData;
    use utoipa::{OpenApi, ToSchema};
    #[derive(OpenApi)]
    #[openapi(components(schemas(
        ApiResponse<NoData, ApiErrorCode, NoData>,
        ApiResponse<ApiEmptyData, ApiErrorCode, NoData>,
        ApiResponse<Profile, ApiErrorCode, Diagnostic>,
        ApiResponse<Task, ApiErrorCode, Diagnostic>
    )))]
    struct TestApi;
    let doc = serde_json::to_value(TestApi::openapi()).unwrap();
    let schemas = doc["components"]["schemas"].as_object().unwrap();
    fn name<T: ToSchema, D: ToSchema>() -> String {
        format!(
            "ApiResponse_{}_{}_{}",
            T::name(),
            ApiErrorCode::name(),
            D::name()
        )
    }
    let no_data = name::<NoData, NoData>();
    let empty = name::<ApiEmptyData, NoData>();
    let profile = name::<Profile, Diagnostic>();
    let task = name::<Task, Diagnostic>();
    assert_eq!(
        [
            no_data.as_str(),
            empty.as_str(),
            profile.as_str(),
            task.as_str()
        ]
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .len(),
        4
    );
    assert_eq!(
        schemas[no_data.as_str()]["properties"]["data"]["type"],
        "null"
    );
    assert_eq!(
        schemas[empty.as_str()]["properties"]["data"]["type"],
        "object"
    );
    assert!(
        schemas[profile.as_str()]["properties"]["data"]["properties"]
            .get("name")
            .is_some()
    );
    assert!(
        schemas[task.as_str()]["properties"]["data"]["properties"]
            .get("completed")
            .is_some()
    );
    fn refs(value: &Value, root: &Value) {
        match value {
            Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                    assert!(root.pointer(&reference[1..]).is_some(), "{reference}");
                }
                for value in map.values() {
                    refs(value, root);
                }
            }
            Value::Array(values) => {
                for value in values {
                    refs(value, root);
                }
            }
            _ => (),
        }
    }
    refs(&doc, &doc);
}
