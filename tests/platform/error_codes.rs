use aster_gate::api::{api_error_code::ApiErrorCode, response::ErrorResponse};

#[test]
fn error_codes_roundtrip_and_have_unique_wire_values() {
    let mut values = std::collections::HashSet::new();
    for &code in ApiErrorCode::ALL {
        assert!(values.insert(code.as_str()));
        assert_eq!(ApiErrorCode::parse(code.as_str()), Some(code));
        assert_eq!(
            serde_json::from_str::<ApiErrorCode>(&serde_json::to_string(&code).unwrap()).unwrap(),
            code
        );
        let response = serde_json::to_value(ErrorResponse::failure(code)).unwrap();
        assert_eq!(response["code"], code.as_str());
        assert_ne!(response["msg"], response["code"]);
        assert!(
            code.status().is_client_error()
                || code.status().is_server_error()
                || code == ApiErrorCode::Success
        );
    }
    assert!(ApiErrorCode::parse("unrecognized").is_none());
}

#[test]
fn authentication_failure_and_session_expiry_remain_distinct() {
    use aster_gate::services::account::AccountError;
    assert_eq!(
        ApiErrorCode::from(&AccountError::Unauthenticated),
        ApiErrorCode::SessionRequired
    );
    assert_eq!(
        ApiErrorCode::from(&AccountError::InvalidCredentials),
        ApiErrorCode::CredentialsFailed
    );
    assert_eq!(
        ApiErrorCode::from(&AccountError::Storage),
        ApiErrorCode::ServiceUnavailable
    );
    assert_eq!(
        ApiErrorCode::from(&AccountError::Security),
        ApiErrorCode::ServiceUnavailable
    );
}
