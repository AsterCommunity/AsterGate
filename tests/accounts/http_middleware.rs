use super::*;
use aster_gate::config::transport::{CorsConfig, HttpSecurityConfig};
use axum::{extract::ConnectInfo, http::HeaderMap, response::Response};
use std::{
    net::SocketAddr,
    num::{NonZeroU32, NonZeroU64},
};

fn request(
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
    peer: Option<&str>,
) -> Request<Body> {
    let mut request = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut request = if let Some(body) = body {
        request
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
    } else {
        request.body(Body::empty())
    }
    .unwrap();
    if let Some(peer) = peer {
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
    }
    request
}

async fn send(f: &Fixture, request: Request<Body>) -> Response {
    f.app.clone().oneshot(request).await.unwrap()
}

fn cookie(response: &Response, name: &str) -> String {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| cookie::Cookie::parse(value.to_str().unwrap()).ok())
        .find(|cookie| cookie.name() == name)
        .unwrap()
        .value()
        .to_owned()
}

fn security_headers(headers: &HeaderMap) {
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(headers["pragma"], "no-cache");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert!(headers.contains_key("x-request-id"));
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert!(headers.contains_key("x-frame-options"));
}

async fn failure(response: Response, status: StatusCode, code: &str) {
    assert_eq!(response.status(), status);
    security_headers(response.headers());
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 100_000).await.unwrap()).unwrap();
    assert_eq!(body["code"], code);
    assert!(body["error"]["retryable"].is_boolean());
}

#[tokio::test]
async fn csrf_bootstrap_setup_rotation_and_logout_clear_are_one_cookie_lifecycle() {
    let f = Fixture::fresh().await;
    let response = send(
        &f,
        request(
            "GET",
            "/api/v1/setup/status",
            &[("host", "localhost:5173")],
            None,
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    security_headers(response.headers());
    let token = cookie(&response, "ag_csrf");
    assert!(token.len() >= 32);
    let raw = response.headers()["set-cookie"].to_str().unwrap();
    assert!(raw.contains("SameSite=Strict") && raw.contains("Path=/"));
    assert!(!raw.contains("Domain=") && !raw.contains("HttpOnly") && !raw.contains("Secure"));
    let pair = format!("ag_csrf={token}");
    let unchanged = send(
        &f,
        request(
            "GET",
            "/api/v1/setup/status",
            &[("host", "localhost:5173"), ("cookie", &pair)],
            None,
            None,
        ),
    )
    .await;
    assert!(!unchanged.headers().contains_key("set-cookie"));
    let setup = send(
        &f,
        request(
            "POST",
            "/api/v1/setup",
            &[
                ("host", "localhost:5173"),
                ("cookie", &pair),
                ("x-astergate-request", "1"),
                ("x-astergate-csrf", &token),
                ("origin", "http://localhost:5173"),
            ],
            Some(setup_input("operator")),
            None,
        ),
    )
    .await;
    assert_eq!(setup.status(), StatusCode::CREATED);
    let rotated = cookie(&setup, "ag_csrf");
    assert_ne!(token, rotated);
    let session = cookie(&setup, "ag_access");
    let pairs = format!("ag_access={session}; ag_csrf={rotated}");
    let logout = send(
        &f,
        request(
            "POST",
            "/api/v1/auth/logout",
            &[
                ("host", "localhost:5173"),
                ("cookie", &pairs),
                ("x-astergate-request", "1"),
                ("x-astergate-csrf", &rotated),
            ],
            None,
            None,
        ),
    )
    .await;
    assert_eq!(logout.status(), StatusCode::OK);
    assert!(cookie(&logout, "ag_csrf").is_empty());
    assert!(cookie(&logout, "ag_access").is_empty());
    assert!(
        logout
            .headers()
            .get_all("set-cookie")
            .iter()
            .all(|value| value.to_str().unwrap().contains("Max-Age=0"))
    );
}

#[tokio::test]
async fn bootstrap_cookie_scheme_tracks_same_host_browser_source_before_and_after_setup() {
    for origin in [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://[::1]:5173",
        "https://localhost:5173",
        "https://127.0.0.1:5173",
        "https://[::1]:5173",
        "https://gate.example.test",
    ] {
        let f = Fixture::fresh().await;
        let host = origin.split_once("://").unwrap().1;
        let referer = format!("{origin}/setup");
        let secure = origin.starts_with("https:");
        let csrf_name = if secure { "__Host-ag_csrf" } else { "ag_csrf" };
        let session_name = if secure {
            "__Host-ag_access"
        } else {
            "ag_access"
        };
        for path in ["/api/v1/setup/status", "/api/v1/public/frontend-config"] {
            let response = send(
                &f,
                request(
                    "GET",
                    path,
                    &[
                        ("host", host),
                        ("referer", &referer),
                        ("sec-fetch-site", "same-origin"),
                        ("x-forwarded-proto", if secure { "http" } else { "https" }),
                    ],
                    None,
                    None,
                ),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK, "{origin}");
            let token = cookie(&response, csrf_name);
            assert!(!token.is_empty());
            let raw = response.headers()["set-cookie"].to_str().unwrap();
            assert_eq!(raw.contains("Secure"), secure);
            let pair = format!("{csrf_name}={token}");
            let retained = send(
                &f,
                request(
                    "GET",
                    path,
                    &[("host", host), ("referer", &referer), ("cookie", &pair)],
                    None,
                    None,
                ),
            )
            .await;
            assert!(!retained.headers().contains_key("set-cookie"));
            let response = send(
                &f,
                request(
                    "POST",
                    "/api/v1/setup",
                    &[
                        ("host", host),
                        ("origin", origin),
                        ("cookie", &pair),
                        ("x-astergate-request", "1"),
                        ("x-astergate-csrf", &token),
                    ],
                    Some(Value::Null),
                    None,
                ),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{origin}");
            assert!(!response.headers().contains_key("set-cookie"));
        }
        let pair = format!("{csrf_name}={CSRF_TOKEN}");
        let mut input = setup_input("operator");
        input["site_urls"] = json!([origin]);
        let response = send(
            &f,
            request(
                "POST",
                "/api/v1/setup",
                &[
                    ("host", host),
                    ("origin", origin),
                    ("cookie", &pair),
                    ("x-astergate-request", "1"),
                    ("x-astergate-csrf", CSRF_TOKEN),
                ],
                Some(input),
                None,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED, "{origin}");
        assert_ne!(cookie(&response, csrf_name), CSRF_TOKEN);
        assert!(!cookie(&response, session_name).is_empty());
        let response = send(
            &f,
            request(
                "GET",
                "/api/v1/setup/status",
                &[("host", host), ("referer", &referer)],
                None,
                None,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!cookie(&response, csrf_name).is_empty());
    }
}

#[tokio::test]
async fn allowed_cors_actual_rate_limit_responses_remain_readable() {
    let origin = "https://client.example.test";
    let mut config = limits(1, 100, &[]);
    config.cors = CorsConfig {
        enabled: true,
        allowed_origins: vec![origin.into()],
    };
    let f = Fixture::with_http_security(false, config).await;
    for (peer, source, allowed) in [
        ("198.51.100.1:1234", origin, true),
        ("198.51.100.2:1234", "https://denied.example.test", false),
    ] {
        for attempt in 0..2 {
            let response = send(
                &f,
                request(
                    "GET",
                    "/api/v1/public/frontend-config",
                    &[("host", "localhost:5173"), ("origin", source)],
                    None,
                    Some(peer),
                ),
            )
            .await;
            security_headers(response.headers());
            assert_eq!(
                response
                    .headers()
                    .contains_key("access-control-allow-origin"),
                allowed
            );
            if allowed {
                assert_eq!(response.headers()["access-control-allow-origin"], origin);
                assert!(
                    response.headers()["access-control-expose-headers"]
                        .to_str()
                        .unwrap()
                        .contains("retry-after")
                );
                if attempt == 0 {
                    assert_eq!(response.status(), StatusCode::OK);
                } else {
                    assert!(
                        response.headers()["vary"]
                            .to_str()
                            .unwrap()
                            .contains("Origin")
                    );
                    assert!(
                        response.headers()["retry-after"]
                            .to_str()
                            .unwrap()
                            .parse::<u64>()
                            .unwrap()
                            > 0
                    );
                    failure(response, StatusCode::TOO_MANY_REQUESTS, "rate_limited").await;
                }
            } else {
                assert!(
                    response.headers()["vary"]
                        .to_str()
                        .unwrap()
                        .contains("Origin")
                );
                if attempt == 0 {
                    failure(response, StatusCode::FORBIDDEN, "auth.forbidden").await;
                } else {
                    failure(response, StatusCode::TOO_MANY_REQUESTS, "rate_limited").await;
                }
            }
        }
    }
}

#[tokio::test]
async fn csrf_missing_duplicate_mismatch_and_source_ambiguity_are_typed_rejections() {
    let f = Fixture::new().await;
    let pair = format!("ag_csrf={CSRF_TOKEN}");
    let duplicate = format!("{pair}; {pair}");
    let oversized = "a".repeat(129);
    for extras in [
        vec![],
        vec![("cookie", pair.as_str())],
        vec![
            ("cookie", pair.as_str()),
            (
                "x-astergate-csrf",
                "other_valid_token_012345678901234567890123",
            ),
        ],
        vec![
            ("cookie", duplicate.as_str()),
            ("x-astergate-csrf", CSRF_TOKEN),
        ],
        vec![
            ("cookie", pair.as_str()),
            ("cookie", pair.as_str()),
            ("x-astergate-csrf", CSRF_TOKEN),
        ],
        vec![
            ("cookie", pair.as_str()),
            ("x-astergate-csrf", CSRF_TOKEN),
            ("x-astergate-csrf", CSRF_TOKEN),
        ],
        vec![
            ("cookie", pair.as_str()),
            ("x-astergate-csrf", oversized.as_str()),
        ],
    ] {
        let mut headers = vec![("host", "localhost:5173"), ("x-astergate-request", "1")];
        headers.extend(extras);
        failure(
            send(
                &f,
                request(
                    "POST",
                    "/api/v1/auth/login",
                    &headers,
                    Some(Value::Null),
                    None,
                ),
            )
            .await,
            StatusCode::FORBIDDEN,
            "auth.forbidden",
        )
        .await;
    }
    for extra in [
        vec![("origin", "https://other.example")],
        vec![("origin", "null")],
        vec![("referer", "https://other.example/page")],
        vec![("sec-fetch-site", "cross-site")],
        vec![
            ("origin", "http://localhost:5173"),
            ("origin", "http://localhost:5173"),
        ],
        vec![
            ("referer", "http://localhost:5173/a"),
            ("referer", "http://localhost:5173/a"),
        ],
        vec![
            ("sec-fetch-site", "same-origin"),
            ("sec-fetch-site", "same-origin"),
        ],
        vec![("x-astergate-request", "1")],
    ] {
        let mut headers = vec![
            ("host", "localhost:5173"),
            ("x-astergate-request", "1"),
            ("cookie", pair.as_str()),
            ("x-astergate-csrf", CSRF_TOKEN),
        ];
        headers.extend(extra);
        failure(
            send(
                &f,
                request(
                    "POST",
                    "/api/v1/auth/login",
                    &headers,
                    Some(Value::Null),
                    None,
                ),
            )
            .await,
            StatusCode::FORBIDDEN,
            "auth.forbidden",
        )
        .await;
    }
    for (method, path) in [
        ("POST", "/api/v1/auth/login"),
        ("POST", "/api/v1/auth/register"),
        ("POST", "/api/v1/auth/verify-email"),
        ("POST", "/api/v1/auth/logout"),
        ("PUT", "/api/v1/management/settings"),
    ] {
        failure(
            send(
                &f,
                request(method, path, &[("x-astergate-request", "1")], None, None),
            )
            .await,
            StatusCode::FORBIDDEN,
            "auth.forbidden",
        )
        .await;
    }
    let accepted = send(
        &f,
        request(
            "POST",
            "/api/v1/auth/login",
            &[
                ("host", "localhost:5173"),
                ("x-astergate-request", "1"),
                ("cookie", &pair),
                ("x-astergate-csrf", CSRF_TOKEN),
                ("referer", "http://localhost:5173/sign-in"),
                ("sec-fetch-site", "same-origin"),
            ],
            Some(Value::Null),
            None,
        ),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn https_host_only_csrf_cookie_rotates_with_login_and_never_uses_forwarded_host() {
    let f = Fixture::fresh().await;
    let mut input = setup_input("operator");
    input["site_urls"] = json!(["https://gate.example.test", "http://localhost:5173"]);
    assert_eq!(
        f.request("POST", "/api/v1/setup", Some(input), None, false)
            .await
            .0,
        StatusCode::CREATED
    );
    let response = send(
        &f,
        request(
            "GET",
            "/api/v1/setup/status",
            &[
                ("host", "gate.example.test"),
                ("forwarded", "host=evil.test;proto=http"),
                ("x-forwarded-proto", "http"),
            ],
            None,
            None,
        ),
    )
    .await;
    let token = cookie(&response, "__Host-ag_csrf");
    let raw = response.headers()["set-cookie"].to_str().unwrap();
    assert!(
        raw.contains("Secure")
            && raw.contains("Path=/")
            && !raw.contains("Domain=")
            && !raw.contains("HttpOnly")
    );
    let pairs = format!("__Host-ag_csrf={token}");
    let response = send(
        &f,
        request(
            "POST",
            "/api/v1/auth/login",
            &[
                ("host", "gate.example.test"),
                ("origin", "https://gate.example.test"),
                ("cookie", &pairs),
                ("x-astergate-request", "1"),
                ("x-astergate-csrf", &token),
            ],
            Some(json!({"identifier":"operator","password":PASSWORD})),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_ne!(cookie(&response, "__Host-ag_csrf"), token);
    assert!(!cookie(&response, "__Host-ag_access").is_empty());
}

#[tokio::test]
async fn runtime_cors_is_explicit_dynamic_and_does_not_authorize_csrf_or_credentials() {
    let f = Fixture::new().await;
    let origin = "https://client.example.test";
    let headers = [
        ("host", "localhost:5173"),
        ("origin", origin),
        ("access-control-request-method", "POST"),
        (
            "access-control-request-headers",
            "content-type,x-astergate-request,x-astergate-csrf",
        ),
    ];
    let disabled = send(
        &f,
        request(
            "GET",
            "/api/v1/public/frontend-config",
            &headers[..2],
            None,
            None,
        ),
    )
    .await;
    assert_eq!(disabled.status(), StatusCode::OK);
    assert!(
        !disabled
            .headers()
            .contains_key("access-control-allow-origin")
    );
    f.state
        .http_security
        .replace_cors(&CorsConfig {
            enabled: true,
            allowed_origins: vec![origin.into()],
        })
        .unwrap();
    let preflight = send(
        &f,
        request("OPTIONS", "/api/v1/auth/login", &headers, None, None),
    )
    .await;
    assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
    assert_eq!(preflight.headers()["access-control-allow-origin"], origin);
    assert!(
        !preflight
            .headers()
            .contains_key("access-control-allow-credentials")
    );
    security_headers(preflight.headers());
    let actual = send(
        &f,
        request(
            "GET",
            "/api/v1/public/frontend-config",
            &headers[..2],
            None,
            None,
        ),
    )
    .await;
    assert_eq!(actual.status(), StatusCode::OK);
    assert_eq!(actual.headers()["access-control-allow-origin"], origin);
    let same_origin = send(
        &f,
        request(
            "GET",
            "/api/v1/public/frontend-config",
            &[
                ("host", "localhost:5173"),
                ("origin", "http://localhost:5173"),
            ],
            None,
            None,
        ),
    )
    .await;
    assert_eq!(same_origin.status(), StatusCode::OK);
    let pair = format!("ag_csrf={CSRF_TOKEN}");
    failure(
        send(
            &f,
            request(
                "POST",
                "/api/v1/auth/login",
                &[
                    ("host", "localhost:5173"),
                    ("origin", origin),
                    ("cookie", &pair),
                    ("x-astergate-request", "1"),
                    ("x-astergate-csrf", CSRF_TOKEN),
                ],
                Some(Value::Null),
                None,
            ),
        )
        .await,
        StatusCode::FORBIDDEN,
        "auth.forbidden",
    )
    .await;
    f.state
        .http_security
        .replace_cors(&CorsConfig {
            enabled: true,
            allowed_origins: vec!["https://replacement.example.test".into()],
        })
        .unwrap();
    failure(
        send(
            &f,
            request("OPTIONS", "/api/v1/auth/login", &headers, None, None),
        )
        .await,
        StatusCode::FORBIDDEN,
        "auth.forbidden",
    )
    .await;
    assert!(
        f.state
            .http_security
            .replace_cors(&CorsConfig {
                enabled: true,
                allowed_origins: vec!["*".into()]
            })
            .is_err()
    );
    assert!(
        f.state
            .http_security
            .replace_cors(&CorsConfig {
                enabled: true,
                allowed_origins: vec![]
            })
            .is_err()
    );
}

fn limits(burst: u32, auth_burst: u32, proxies: &[&str]) -> HttpSecurityConfig {
    let mut config = HttpSecurityConfig {
        trusted_proxies: proxies.iter().map(|v| (*v).to_owned()).collect(),
        ..Default::default()
    };
    config.rate_limit.seconds_per_request = NonZeroU64::new(60).unwrap();
    config.rate_limit.burst = NonZeroU32::new(burst).unwrap();
    config.rate_limit.authentication_seconds_per_request = NonZeroU64::new(60).unwrap();
    config.rate_limit.authentication_burst = NonZeroU32::new(auth_burst).unwrap();
    config
}

#[tokio::test]
async fn trusted_proxy_ipv4_ipv6_and_missing_peer_keep_explicit_bucket_boundaries() {
    let f = Fixture::with_http_security(false, limits(1, 100, &["10.0.0.0/8", "fd00::/8"])).await;
    let cases = [
        (Some("198.51.100.1:1234"), "203.0.113.1", StatusCode::OK),
        (
            Some("198.51.100.1:1234"),
            "203.0.113.2",
            StatusCode::TOO_MANY_REQUESTS,
        ),
        (Some("198.51.100.2:1234"), "203.0.113.1", StatusCode::OK),
        (
            Some("10.0.0.1:1234"),
            "203.0.113.10, 10.0.0.2",
            StatusCode::OK,
        ),
        (
            Some("10.0.0.1:1234"),
            "203.0.113.11, 10.0.0.2",
            StatusCode::OK,
        ),
        (
            Some("10.0.0.2:1234"),
            "203.0.113.10",
            StatusCode::TOO_MANY_REQUESTS,
        ),
        (Some("[fd00::1]:1234"), "2001:db8::10", StatusCode::OK),
        (Some("[fd00::1]:1234"), "2001:db8::11", StatusCode::OK),
        (
            Some("[fd00::2]:1234"),
            "2001:db8::10",
            StatusCode::TOO_MANY_REQUESTS,
        ),
        (Some("[2001:db8::99]:1234"), "2001:db8::20", StatusCode::OK),
        (
            Some("[2001:db8::99]:1234"),
            "2001:db8::21",
            StatusCode::TOO_MANY_REQUESTS,
        ),
        (None, "203.0.113.100", StatusCode::OK),
        (None, "203.0.113.101", StatusCode::TOO_MANY_REQUESTS),
    ];
    for (peer, forwarded, status) in cases {
        let response = send(
            &f,
            request(
                "GET",
                "/api/v1/setup/status",
                &[("x-forwarded-for", forwarded)],
                None,
                peer,
            ),
        )
        .await;
        assert_eq!(response.status(), status, "{peer:?}: {forwarded}");
        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait: u64 = response.headers()["retry-after"]
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            assert!((1..=60).contains(&wait));
            failure(response, status, "rate_limited").await;
        }
    }
}

#[tokio::test]
async fn http_limits_are_shared_across_router_clones_and_enforce_concurrent_burst() {
    let f = Fixture::with_http_security(false, limits(3, 100, &[])).await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..12 {
        let router = f.app.clone();
        tasks.spawn(async move {
            router
                .oneshot(request(
                    "GET",
                    "/api/v1/setup/status",
                    &[],
                    None,
                    Some("198.51.100.1:1234"),
                ))
                .await
                .unwrap()
        });
    }
    let mut allowed = 0;
    while let Some(result) = tasks.join_next().await {
        let response = result.unwrap();
        if response.status() == StatusCode::OK {
            allowed += 1;
        } else {
            failure(response, StatusCode::TOO_MANY_REQUESTS, "rate_limited").await;
        }
    }
    assert_eq!(allowed, 3);
    let rebuilt = aster_gate::api::router(f.state.clone());
    assert_eq!(
        rebuilt
            .oneshot(request(
                "GET",
                "/api/v1/setup/status",
                &[],
                None,
                Some("198.51.100.1:1234")
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        send(
            &f,
            request("GET", "/health", &[], None, Some("198.51.100.1:1234"))
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn authentication_keyed_quota_is_separate_from_general_ip_and_counts_preflight() {
    let mut config = limits(100, 1, &["10.0.0.0/8"]);
    config.cors = CorsConfig {
        enabled: true,
        allowed_origins: vec!["https://client.example.test".into()],
    };
    let f = Fixture::with_http_security(false, config).await;
    let preflight = [
        ("origin", "https://client.example.test"),
        ("access-control-request-method", "POST"),
        ("x-forwarded-for", "203.0.113.20"),
    ];
    let response = send(
        &f,
        request(
            "OPTIONS",
            "/api/v1/auth/login",
            &preflight,
            None,
            Some("10.0.0.1:1234"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = send(
        &f,
        request(
            "OPTIONS",
            "/api/v1/auth/login",
            &preflight,
            None,
            Some("10.0.0.2:1234"),
        ),
    )
    .await;
    assert_eq!(
        response.headers()["access-control-allow-origin"],
        "https://client.example.test"
    );
    assert!(response.headers().contains_key("retry-after"));
    assert!(
        !response
            .headers()
            .contains_key("access-control-allow-credentials")
    );
    failure(response, StatusCode::TOO_MANY_REQUESTS, "rate_limited").await;
    assert_eq!(
        send(
            &f,
            request(
                "GET",
                "/api/v1/setup/status",
                &[("x-forwarded-for", "203.0.113.20")],
                None,
                Some("10.0.0.1:1234")
            )
        )
        .await
        .status(),
        StatusCode::OK
    );
    let other = [
        ("origin", "https://client.example.test"),
        ("access-control-request-method", "POST"),
        ("x-forwarded-for", "203.0.113.21"),
    ];
    assert_eq!(
        send(
            &f,
            request(
                "OPTIONS",
                "/api/v1/auth/login",
                &other,
                None,
                Some("10.0.0.1:1234")
            )
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
}

#[test]
fn transport_configuration_rejects_invalid_limits_and_ambiguous_policy() {
    use aster_gate::api::middleware::HttpSecurity;
    for value in ["not-an-ip", "0.0.0.0/33", "2001:db8::/129"] {
        assert!(
            HttpSecurity::new(&HttpSecurityConfig {
                trusted_proxies: vec![value.into()],
                ..Default::default()
            })
            .is_err()
        );
    }
    assert!(
        HttpSecurity::new(&HttpSecurityConfig {
            cors: CorsConfig {
                enabled: true,
                allowed_origins: vec!["https://*.example.test".into()]
            },
            ..Default::default()
        })
        .is_err()
    );
    for value in [
        json!({"rate_limit":{"burst":0}}),
        json!({"rate_limit":{"seconds_per_request":0}}),
        json!({"rate_limit":{"authentication_burst":0}}),
    ] {
        assert!(serde_json::from_value::<HttpSecurityConfig>(value).is_err());
    }
    for (seconds, burst, authentication) in [
        (u64::MAX, 1, false),
        (u64::MAX / 1_000_000_000 + 1, 1, false),
        (u64::MAX / 1_000_000_000, 2, false),
        (u64::MAX, 1, true),
        (u64::MAX / 1_000_000_000, 2, true),
    ] {
        let mut config = HttpSecurityConfig::default();
        if authentication {
            config.rate_limit.authentication_seconds_per_request =
                NonZeroU64::new(seconds).unwrap();
            config.rate_limit.authentication_burst = NonZeroU32::new(burst).unwrap();
        } else {
            config.rate_limit.seconds_per_request = NonZeroU64::new(seconds).unwrap();
            config.rate_limit.burst = NonZeroU32::new(burst).unwrap();
        }
        assert!(HttpSecurity::new(&config).is_err());
    }
}

#[tokio::test]
async fn cross_site_public_reads_do_not_mint_or_replace_csrf_cookies() {
    let f = Fixture::new().await;
    for headers in [
        vec![
            ("host", "localhost:5173"),
            ("origin", "https://other.example"),
        ],
        vec![("host", "localhost:5173"), ("sec-fetch-site", "cross-site")],
    ] {
        let response = send(
            &f,
            request("GET", "/api/v1/setup/status", &headers, None, None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key("set-cookie"));
    }
}

#[tokio::test]
async fn http_quota_refills_at_configured_boundary_and_can_be_disabled() {
    let mut config = limits(1, 100, &[]);
    config.rate_limit.seconds_per_request = NonZeroU64::MIN;
    let f = Fixture::with_http_security(false, config).await;
    assert_eq!(
        send(&f, request("GET", "/api/v1/setup/status", &[], None, None))
            .await
            .status(),
        StatusCode::OK
    );
    let rejected = send(&f, request("GET", "/api/v1/setup/status", &[], None, None)).await;
    assert_eq!(rejected.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(rejected.headers()["retry-after"], "1");
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(
        send(&f, request("GET", "/api/v1/setup/status", &[], None, None))
            .await
            .status(),
        StatusCode::OK
    );
    let mut config = limits(1, 1, &[]);
    config.rate_limit.enabled = false;
    let f = Fixture::with_http_security(false, config).await;
    for _ in 0..4 {
        assert_eq!(
            send(&f, request("GET", "/api/v1/setup/status", &[], None, None))
                .await
                .status(),
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn failed_session_transaction_does_not_issue_or_rotate_cookies() {
    let f = Fixture::new().await;
    f.reject_inserts("browser_sessions").await;
    let pair = format!("ag_csrf={CSRF_TOKEN}");
    let response = send(
        &f,
        request(
            "POST",
            "/api/v1/auth/login",
            &[
                ("host", "localhost:5173"),
                ("cookie", &pair),
                ("x-astergate-csrf", CSRF_TOKEN),
                ("x-astergate-request", "1"),
            ],
            Some(json!({"identifier":"instance-operator","password":PASSWORD})),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key("set-cookie"));
    security_headers(response.headers());
}

#[tokio::test]
async fn frontend_product_csp_and_cache_policy_survive_outer_middlewares() {
    let f = Fixture::fresh().await;
    let response = send(&f, request("GET", "/sign-in", &[], None, None)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-security-policy"],
        aster_gate::api::routes::frontend::FRONTEND_CSP_HEADER
    );
    assert_eq!(response.headers()["cache-control"], "no-cache");
    assert!(response.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn bootstrap_sources_cannot_change_authority_or_bypass_source_policy() {
    let config = HttpSecurityConfig {
        cors: CorsConfig {
            enabled: true,
            allowed_origins: vec!["https://client.example.test".into()],
        },
        ..Default::default()
    };
    let f = Fixture::with_http_security(false, config).await;
    let pair = format!("__Host-ag_csrf={CSRF_TOKEN}");
    for extra in [
        vec![("referer", "http://localhost:5173/setup")],
        vec![
            ("origin", "https://localhost:5173"),
            ("sec-fetch-site", "cross-site"),
        ],
        vec![
            ("origin", "https://other.example.test"),
            ("referer", "https://localhost:5173/setup"),
        ],
        vec![
            ("origin", "https://localhost:5173"),
            ("origin", "https://localhost:5173"),
        ],
    ] {
        let mut headers = vec![
            ("host", "localhost:5173"),
            ("cookie", &pair),
            ("x-astergate-request", "1"),
            ("x-astergate-csrf", CSRF_TOKEN),
        ];
        headers.extend(extra);
        let response = send(
            &f,
            request("POST", "/api/v1/setup", &headers, Some(Value::Null), None),
        )
        .await;
        assert!(!response.headers().contains_key("set-cookie"));
        failure(response, StatusCode::FORBIDDEN, "auth.forbidden").await;
    }
    // Same-host HTTPS is same-origin even with an unrelated explicit CORS allowlist.
    let response = send(
        &f,
        request(
            "POST",
            "/api/v1/setup",
            &[
                ("host", "localhost:5173"),
                ("origin", "https://localhost:5173"),
                ("cookie", &pair),
                ("x-astergate-request", "1"),
                ("x-astergate-csrf", CSRF_TOKEN),
            ],
            Some(Value::Null),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn route_groups_keep_bootstrap_public_setup_protected_and_accounts_not_ready() {
    let f = Fixture::fresh().await;
    for path in ["/api/v1/setup/status", "/api/v1/public/frontend-config"] {
        let response = send(
            &f,
            request("GET", path, &[("host", "localhost:5173")], None, None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!cookie(&response, "ag_csrf").is_empty());
    }
    failure(
        send(
            &f,
            request(
                "POST",
                "/api/v1/setup",
                &[("host", "localhost:5173")],
                Some(setup_input("operator")),
                None,
            ),
        )
        .await,
        StatusCode::FORBIDDEN,
        "auth.forbidden",
    )
    .await;
    let pair = format!("ag_csrf={CSRF_TOKEN}");
    for (method, path, body) in [
        ("GET", "/api/v1/account", None),
        ("GET", "/api/v1/management/accounts", None),
        ("POST", "/api/v1/auth/login", Some(Value::Null)),
        ("POST", "/api/v1/auth/register", Some(Value::Null)),
    ] {
        let response = send(
            &f,
            request(
                method,
                path,
                &[
                    ("host", "localhost:5173"),
                    ("cookie", &pair),
                    ("x-astergate-request", "1"),
                    ("x-astergate-csrf", CSRF_TOKEN),
                ],
                body,
                None,
            ),
        )
        .await;
        assert!(!response.headers().contains_key("set-cookie"));
        failure(response, StatusCode::CONFLICT, "setup.required").await;
    }
}

#[tokio::test]
async fn session_cookie_pairs_follow_committed_revocation_and_audit_rollback() {
    for operation in [
        "logout", "current", "all", "other", "password", "email", "delete",
    ] {
        for fail_audit in [false, true] {
            let f = Fixture::new().await;
            let (_, session) = f.activate("alice").await;
            let pair = format!("{session}; ag_csrf={CSRF_TOKEN}");
            let (method, path, body) = match operation {
                "logout" => ("POST", "/api/v1/auth/logout".into(), None),
                "all" => ("DELETE", "/api/v1/account/sessions".into(), None),
                "current" | "other" => {
                    if operation == "other" {
                        f.login("alice", PASSWORD).await;
                    }
                    let (_, data, _) = f
                        .request(
                            "GET",
                            "/api/v1/account/sessions",
                            None,
                            Some(&session),
                            false,
                        )
                        .await;
                    let row = data["data"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|s| s["current"] == (operation == "current"))
                        .unwrap();
                    (
                        "DELETE",
                        format!("/api/v1/account/sessions/{}", row["id"].as_str().unwrap()),
                        None,
                    )
                }
                "password" => (
                    "POST",
                    "/api/v1/account/password".into(),
                    Some(json!({"current_password":PASSWORD, "new_password":NEW_PASSWORD})),
                ),
                "email" => {
                    assert_eq!(
                        f.request(
                            "POST",
                            "/api/v1/account/email",
                            Some(json!({"email":"replacement@example.test", "password":PASSWORD})),
                            Some(&session),
                            false
                        )
                        .await
                        .0,
                        StatusCode::OK
                    );
                    (
                        "POST",
                        "/api/v1/account/email/confirm".into(),
                        Some(json!({"token": f.token().await})),
                    )
                }
                "delete" => (
                    "DELETE",
                    "/api/v1/account".into(),
                    Some(json!({"password":PASSWORD})),
                ),
                _ => unreachable!(),
            };
            if fail_audit {
                f.reject_inserts("audit_logs").await;
            }
            let response = send(
                &f,
                request(
                    method,
                    &path,
                    &[
                        ("host", "localhost:5173"),
                        ("cookie", &pair),
                        ("x-astergate-request", "1"),
                        ("x-astergate-csrf", CSRF_TOKEN),
                    ],
                    body,
                    None,
                ),
            )
            .await;
            assert_eq!(
                response.status(),
                if fail_audit {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::OK
                },
                "{operation} audit_failure={fail_audit}"
            );
            if fail_audit || operation == "other" {
                assert!(
                    !response.headers().contains_key("set-cookie"),
                    "{operation}"
                );
                assert_eq!(
                    f.request("GET", "/api/v1/account", None, Some(&session), false)
                        .await
                        .0,
                    StatusCode::OK
                );
            } else {
                assert!(cookie(&response, "ag_access").is_empty());
                assert!(cookie(&response, "ag_csrf").is_empty());
                assert_eq!(response.headers().get_all("set-cookie").iter().count(), 3);
                assert!(
                    response
                        .headers()
                        .get_all("set-cookie")
                        .iter()
                        .all(|value| value.to_str().unwrap().contains("Max-Age=0"))
                );
                assert_eq!(
                    f.request("GET", "/api/v1/account", None, Some(&session), false)
                        .await
                        .0,
                    StatusCode::UNAUTHORIZED
                );
            }
        }
    }
}

#[tokio::test]
async fn setup_audit_failure_does_not_issue_either_cookie() {
    let f = Fixture::fresh().await;
    f.reject_inserts("audit_logs").await;
    let pair = format!("ag_csrf={CSRF_TOKEN}");
    let response = send(
        &f,
        request(
            "POST",
            "/api/v1/setup",
            &[
                ("host", "localhost:5173"),
                ("cookie", &pair),
                ("x-astergate-request", "1"),
                ("x-astergate-csrf", CSRF_TOKEN),
            ],
            Some(setup_input("operator")),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key("set-cookie"));
}
