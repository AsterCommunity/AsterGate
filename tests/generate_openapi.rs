#![cfg(all(debug_assertions, feature = "openapi"))]
//! OpenAPI generation test.

use std::fs;

use aster_gate::api::openapi::ApiDoc;
use utoipa::OpenApi;

#[test]
fn generate_openapi() {
    let mut doc = ApiDoc::openapi();
    doc.info.title = "AsterGate".to_string();
    doc.info.description = Some("AsterGate Account and management API.".to_string());
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(&doc).expect("serialize openapi document")
    );

    fs::create_dir_all("./frontend-panel/generated").expect("create frontend generated directory");
    fs::write("./frontend-panel/generated/openapi.json", json)
        .expect("write frontend OpenAPI spec");
}

#[test]
fn generated_openapi_contains_health_route() {
    let doc = ApiDoc::openapi();
    let value = serde_json::to_value(&doc).expect("openapi json value");

    assert!(value["paths"].get("/health").is_some());
    assert!(value["paths"].get("/health/ready").is_some());
    assert!(
        value["components"]["schemas"]
            .get("StatusResponse")
            .is_some()
    );
    assert!(
        value["components"]["schemas"]
            .get("ErrorResponse")
            .is_some()
    );
}

#[test]
fn account_contract_documents_management_and_session_security_without_exposing_passwords() {
    let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
    assert_eq!(
        doc["paths"]["/api/v1/account"]["get"]["security"][0],
        serde_json::json!({"SessionCookie":[]})
    );
    assert_eq!(
        doc["paths"]["/api/v1/management/accounts"]["get"]["security"][0],
        serde_json::json!({"SessionCookie":[]})
    );
    assert!(
        doc["paths"]["/api/v1/auth/register"]["post"]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "X-AsterGate-Request" && p["required"] == true)
    );
    let response = doc["components"]["schemas"]["AccountDto"].to_string();
    let permissions = doc["components"]["schemas"]["SystemPermissionDto"]["enum"]
        .as_array()
        .unwrap();
    let registered = aster_gate::services::authorization::models::SystemPermission::ALL;
    assert_eq!(permissions.len(), registered.len());
    for permission in registered {
        assert!(permissions.contains(&serde_json::json!(permission.as_str())));
    }
    assert!(
        doc["components"]["schemas"]["AccountDto"]["properties"]
            .get("capabilities")
            .is_none()
    );
    assert_eq!(
        doc["components"]["schemas"]["AccountDto"]["properties"]["system_permissions"]["items"]["$ref"],
        "#/components/schemas/SystemPermissionDto"
    );
    let codes: Vec<String> = doc["components"]["schemas"]["ApiErrorCode"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let expected: Vec<String> = aster_gate::api::api_error_code::ApiErrorCode::ALL
        .iter()
        .map(|v| v.as_str().to_string())
        .collect();
    assert_eq!(codes.len(), expected.len());
    assert!(expected.iter().all(|v| codes.contains(v)));
    assert_eq!(
        doc["components"]["schemas"]["AccountResponse"]["properties"]["data"]["$ref"],
        "#/components/schemas/AccountDto"
    );
    assert_eq!(
        doc["components"]["schemas"]["SettingsResponse"]["properties"]["data"]["$ref"],
        "#/components/schemas/SettingsDto"
    );
    assert_eq!(
        doc["components"]["schemas"]["SessionsResponse"]["properties"]["data"]["type"],
        "array"
    );
    assert_eq!(
        doc["components"]["schemas"]["ErrorResponse"]["properties"]["code"]["$ref"],
        "#/components/schemas/ApiErrorCode"
    );
    for forbidden in [
        "password_hash",
        "management_token",
        "organization_id",
        "token_hash",
    ] {
        assert!(!response.contains(forbidden));
    }
    for path in [
        "/.well-known/openid-configuration",
        "/oauth/token",
        "/jwks",
        "/api/v1/organizations",
    ] {
        assert!(doc["paths"].get(path).is_none());
    }
}
