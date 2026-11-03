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
            .get("ApiResponse_TupleUnit_ApiErrorCode_TupleUnit")
            .is_some()
    );
}

#[test]
fn account_contract_documents_management_and_session_security_without_exposing_passwords() {
    let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
    assert_eq!(
        doc["paths"]["/api/v1/auth/refresh"]["post"]["security"][0],
        serde_json::json!({"RefreshCookie":[], "CsrfCookie":[]})
    );
    assert_eq!(
        doc["paths"]["/api/v1/auth/logout"]["post"]["security"][2],
        serde_json::json!({"SessionCookie":[], "CsrfCookie":[]})
    );
    for (scheme, name) in [
        ("SessionCookie", "__Host-ag_access"),
        ("RefreshCookie", "__Secure-ag_refresh"),
    ] {
        assert_eq!(doc["components"]["securitySchemes"][scheme]["name"], name);
    }
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
    assert_eq!(
        doc["paths"]["/api/v1/account"]["patch"]["security"][0],
        serde_json::json!({"SessionCookie":[], "CsrfCookie":[]})
    );
    assert_eq!(
        doc["paths"]["/api/v1/auth/register"]["post"]["security"][1],
        serde_json::json!({"LocalCsrfCookie":[]})
    );
    assert!(
        doc["paths"]["/api/v1/account"]["get"]["responses"]["429"]["headers"]
            .get("Retry-After")
            .is_some()
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
    let schema_for = |path: &str, method: &str, status: &str| {
        let reference = doc["paths"][path][method]["responses"][status]["content"]["application/json"]["schema"]["$ref"].as_str().unwrap();
        (reference, doc.pointer(&reference[1..]).unwrap())
    };
    let (account_ref, account) = schema_for("/api/v1/account", "get", "200");
    let (settings_ref, settings) = schema_for("/api/v1/management/settings", "get", "200");
    let (sessions_ref, sessions) = schema_for("/api/v1/account/sessions", "get", "200");
    assert_ne!(account_ref, settings_ref);
    assert_ne!(account_ref, sessions_ref);
    assert!(
        account["properties"]["data"]["properties"]
            .get("username")
            .is_some()
    );
    assert!(
        account["properties"]["data"]["properties"]
            .get("revision")
            .is_none()
    );
    assert!(
        settings["properties"]["data"]["properties"]
            .get("revision")
            .is_some()
    );
    assert_eq!(sessions["properties"]["data"]["type"], "array");
    let (_, accounts) = schema_for("/api/v1/management/accounts", "get", "200");
    let cursor = &accounts["properties"]["data"]["properties"]["next_cursor"];
    fn json_types(schema: &serde_json::Value) -> std::collections::BTreeSet<&str> {
        let mut types = std::collections::BTreeSet::new();
        if let Some(kind) = schema["type"].as_str() {
            types.insert(kind);
        }
        if let Some(kinds) = schema["type"].as_array() {
            types.extend(kinds.iter().filter_map(serde_json::Value::as_str));
        }
        for composition in ["oneOf", "anyOf"] {
            if let Some(schemas) = schema[composition].as_array() {
                for schema in schemas {
                    types.extend(json_types(schema));
                }
            }
        }
        types
    }
    assert_eq!(json_types(cursor), ["null", "string"].into_iter().collect());
    assert!(doc["components"]["schemas"].get("String").is_none());
    let (_, failure) = schema_for("/api/v1/account", "get", "401");
    assert_eq!(
        failure["properties"]["code"]["$ref"],
        "#/components/schemas/ApiErrorCode"
    );
    for old in [
        "AccountResponse",
        "SettingsResponse",
        "SessionsResponse",
        "ErrorResponse",
        "SuccessResponse",
    ] {
        assert!(
            doc["components"]["schemas"].get(old).is_none(),
            "obsolete wrapper {old}"
        );
    }
    for (path, item) in doc["paths"].as_object().unwrap() {
        if !path.starts_with("/api/v1/") {
            continue;
        }
        for method in ["post", "put", "patch", "delete"] {
            let Some(operation) = item.get(method) else {
                continue;
            };
            assert!(
                operation["parameters"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p["name"] == "X-AsterGate-CSRF" && p["required"] == true),
                "{method} {path}"
            );
        }
    }
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

#[test]
fn every_generated_schema_reference_resolves() {
    fn check(value: &serde_json::Value, root: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(|v| v.as_str()) {
                    assert!(reference.starts_with("#/"));
                    assert!(
                        root.pointer(&reference[1..]).is_some(),
                        "unresolved {reference}"
                    );
                }
                for child in map.values() {
                    check(child, root);
                }
            }
            serde_json::Value::Array(array) => {
                for child in array {
                    check(child, root);
                }
            }
            _ => (),
        }
    }
    let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
    check(&doc, &doc);
}
