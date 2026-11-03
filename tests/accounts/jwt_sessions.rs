//! Gate/Forge integration against the real router and the configured writer backend.
use super::*;
use aster_forge_auth::{
    AuthTokenType, ClientContext, Clock, JwtKey, JwtPolicy, JwtSigner, JwtVerifier, Restriction,
    SessionVersion, Subject,
};
use aster_gate::services::account::{self as service, AccountError};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

struct TestClock(AtomicI64, AtomicUsize);
impl Clock for TestClock {
    /// Counts clock reads so tests can observe entry into writer waits without timing-based sleeps.
    fn now_unix_seconds(&self) -> aster_forge_auth::Result<i64> {
        self.1.fetch_add(1, Ordering::SeqCst);
        Ok(self.0.load(Ordering::SeqCst))
    }
}

/// Installs one deterministic clock on the fixture runtime and rebuilds its real router.
fn clock(f: &mut Fixture, now: i64) -> Arc<TestClock> {
    let clock = Arc::new(TestClock(AtomicI64::new(now), AtomicUsize::new(0)));
    let mut state = (*f.state).clone();
    let mut security = (*state.account_security).clone();
    security.sessions = security.sessions.with_clock(clock.clone());
    state.account_security = Arc::new(security);
    f.state = Arc::new(state);
    f.app = aster_gate::api::router(f.state.clone());
    clock
}

/// Signs in through the product service to obtain a persisted, committed session credential pair.
async fn pair(f: &Fixture) -> aster_forge_auth::AuthTokenPair {
    service::login(&f.state, "instance-operator", PASSWORD.into(), "browser")
        .await
        .unwrap()
        .tokens
}

/// Verifies a fixture credential against its explicit purpose/audience before inspecting claims.
fn claims(f: &Fixture, token: &str, kind: AuthTokenType) -> aster_forge_auth::TokenClaims {
    let audience = match kind {
        AuthTokenType::Access => "aster-gate-web-access",
        AuthTokenType::Refresh => "aster-gate-web-refresh",
    };
    JwtVerifier::with_system_clock(
        JwtKey::hmac(hex::decode(&f.state.config.session.signing_key).unwrap()).unwrap(),
        JwtPolicy::new(&f.state.config.session.issuer, audience, 300, 86400).unwrap(),
    )
    .verify(token, kind)
    .unwrap()
    .into_claims()
}

/// Calls the product refresh adapter with fixed browser metadata so tests exercise Strict policy.
async fn refresh(f: &Fixture, token: &str) -> service::Result<aster_forge_auth::AuthTokenPair> {
    service::sessions::refresh(
        &f.state,
        token,
        ClientContext {
            user_agent: Some("browser".into()),
            ip_address: None,
        },
    )
    .await
}

/// Sends a real refresh/logout router request with matching CSRF and trusted loopback Host headers.
async fn request(f: &Fixture, path: &str, cookies: &str) -> axum::response::Response {
    f.app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("host", "localhost:5173")
                .header("cookie", format!("{cookies}; ag_csrf={CSRF_TOKEN}"))
                .header("x-astergate-request", "1")
                .header("x-astergate-csrf", CSRF_TOKEN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

/// Extracts one issued Cookie value for protocol assertions without relying on header ordering.
fn cookie(response: &axum::response::Response, name: &str) -> String {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .find_map(|header| {
            let cookie = cookie::Cookie::parse(header.to_str().unwrap()).unwrap();
            (cookie.name() == name).then(|| cookie.value().to_owned())
        })
        .unwrap()
}

/// Checks credential-purpose substitution, required claim shapes and Model/ActiveModel/token-pair redaction.
#[tokio::test]
async fn pair_has_distinct_namespaces_typed_claims_and_no_plaintext_api_tokens() {
    let f = Fixture::new().await;
    let pair = pair(&f).await;
    let access = claims(&f, &pair.access_token, AuthTokenType::Access);
    let refresh_claims = claims(&f, &pair.refresh_token, AuthTokenType::Refresh);
    assert_eq!(access.sub.as_str(), f.initial_account_id);
    assert_eq!(access.sid, refresh_claims.sid);
    assert!(access.jti.is_none() && refresh_claims.jti.is_some());
    assert_ne!(access.aud, refresh_claims.aud);
    assert_eq!(access.exp - access.iat, 300);
    assert_eq!(refresh_claims.exp - refresh_claims.iat, 86400);
    assert!(
        service::sessions::authenticate(&f.state, &pair.refresh_token)
            .await
            .is_err()
    );
    assert!(refresh_token_as_access(&f, &pair.refresh_token).await);
    assert!(matches!(
        refresh(&f, &pair.access_token).await,
        Err(AccountError::Unauthenticated)
    ));
    let row = session::Entity::find_by_id(access.sid.as_str())
        .one(f.state.db_handles.writer())
        .await
        .unwrap()
        .unwrap();
    use sea_orm::IntoActiveModel;
    let debug = format!("{row:?} {pair:?} {:?}", row.clone().into_active_model());
    assert!(!debug.contains(&pair.access_token) && !debug.contains(&pair.refresh_token));
    assert!(!debug.contains(row.current_refresh_jti.as_str()));
}

/// Checks through the router that a refresh JWT cannot authenticate the Account endpoint.
async fn refresh_token_as_access(f: &Fixture, token: &str) -> bool {
    f.request(
        "GET",
        "/api/v1/account",
        None,
        Some(&format!("ag_access={token}")),
        false,
    )
    .await
    .0 == StatusCode::UNAUTHORIZED
}

/// Advances trusted time through access and refresh expiry to prove rotation cannot extend the deadline.
/// Also checks unchanged business revision, security epoch and CSRF Cookie.
#[tokio::test]
async fn rotation_preserves_absolute_deadline_revision_epoch_and_csrf() {
    let mut f = Fixture::new().await;
    let initial = pair(&f).await;
    let original = claims(&f, &initial.refresh_token, AuthTokenType::Refresh);
    let db = f.state.db_handles.writer().clone();
    let before = account::Entity::find_by_id(&f.initial_account_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let clock = clock(&mut f, original.iat + 301);
    assert!(matches!(
        service::sessions::authenticate(&f.state, &initial.access_token).await,
        Err(AccountError::AccessExpired)
    ));
    let response = request(
        &f,
        "/api/v1/auth/refresh",
        &format!("ag_refresh={}", initial.refresh_token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let access = cookie(&response, "ag_access");
    let next = cookie(&response, "ag_refresh");
    assert_ne!(next, initial.refresh_token);
    assert_eq!(response.headers().get_all("set-cookie").iter().count(), 2);
    assert!(
        response
            .headers()
            .get_all("set-cookie")
            .iter()
            .all(|v| !v.to_str().unwrap().starts_with("ag_csrf="))
    );
    assert!(
        service::sessions::authenticate(&f.state, &access)
            .await
            .is_ok()
    );
    let after = account::Entity::find_by_id(&f.initial_account_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (before.revision, before.session_version),
        (after.revision, after.session_version)
    );
    let row = session::Entity::find_by_id(original.sid.as_str())
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.expires_at, original.exp);
    clock.0.store(original.exp - 1, Ordering::SeqCst);
    let last = refresh(&f, &next).await.unwrap();
    assert_eq!(last.refresh_expires_at, original.exp);
    assert_eq!(last.access_expires_at, original.exp);
    clock.0.store(original.exp, Ordering::SeqCst);
    assert!(refresh(&f, &last.refresh_token).await.is_err());
    assert!(
        service::sessions::authenticate(&f.state, &last.access_token)
            .await
            .is_err()
    );
}

/// Replays a rotated credential through the router and verifies committed epoch/revocation/audit effects.
/// The rejection must not issue Cookies or invalidate an unrelated Account.
#[tokio::test]
async fn strict_reuse_commits_global_revocation_but_does_not_touch_other_accounts() {
    let f = Fixture::new().await;
    let (_, other) = f.activate("alice").await;
    let initial = pair(&f).await;
    let second = pair(&f).await;
    let next = refresh(&f, &initial.refresh_token).await.unwrap();
    let response = request(
        &f,
        "/api/v1/auth/refresh",
        &format!("ag_refresh={}", initial.refresh_token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(!response.headers().contains_key("set-cookie"));
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap()).unwrap();
    assert_eq!(body["code"], "auth.refresh_reused");
    for token in [
        &initial.access_token,
        &second.access_token,
        &next.access_token,
    ] {
        assert!(matches!(
            service::sessions::authenticate(&f.state, token).await,
            Err(AccountError::Unauthenticated)
        ));
    }
    assert!(refresh(&f, &next.refresh_token).await.is_err());
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&other), false)
            .await
            .0,
        StatusCode::OK
    );
    let row = account::Entity::find_by_id(&f.initial_account_id)
        .one(f.state.db_handles.writer())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.session_version, 1);
    let audits = aster_forge_db::audit_log::Entity::find()
        .all(f.state.db_handles.writer())
        .await
        .unwrap();
    let reused = audits
        .iter()
        .filter(|a| a.action == "account.refresh_reused")
        .collect::<Vec<_>>();
    assert_eq!(reused.len(), 1);
    let details = reused[0].details.as_ref().unwrap();
    assert!(
        !details.contains(&initial.refresh_token)
            && !details.contains(initial.refresh_jti.as_str())
    );
}

/// Keeps JWTs unexpired while proving profile edits preserve sessions and one sid revoke denies both types.
#[tokio::test]
async fn single_revocation_is_immediate_and_profile_edits_preserve_other_sessions() {
    let f = Fixture::new().await;
    let first = pair(&f).await;
    let second = pair(&f).await;
    let principal = service::sessions::authenticate(&f.state, &first.access_token)
        .await
        .unwrap();
    service::update_profile(&f.state, &principal, "Updated")
        .await
        .unwrap();
    assert!(
        service::sessions::authenticate(&f.state, &first.access_token)
            .await
            .is_ok()
    );
    let principal = service::sessions::authenticate(&f.state, &first.access_token)
        .await
        .unwrap();
    let sid = claims(&f, &second.access_token, AuthTokenType::Access).sid;
    service::revoke_session(
        &f.state,
        &principal,
        &service::models::SessionId::parse(sid.as_str()).unwrap(),
    )
    .await
    .unwrap();
    assert!(
        service::sessions::authenticate(&f.state, &second.access_token)
            .await
            .is_err()
    );
    assert!(refresh(&f, &second.refresh_token).await.is_err());
    assert!(
        service::sessions::authenticate(&f.state, &first.access_token)
            .await
            .is_ok()
    );
}

/// Starts two runtimes against one database and releases concurrent refreshes through a barrier.
/// Exactly one rotation succeeds; the Strict loser commits compromise revocation.
#[tokio::test]
async fn independent_instance_coordinators_have_one_committed_database_winner() {
    let f = Fixture::new().await;
    let initial = pair(&f).await;
    let replica = aster_gate::runtime::assembly::prepare_state((*f.state.config).clone())
        .await
        .unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let left = {
        let barrier = barrier.clone();
        let state = f.state.clone();
        let token = initial.refresh_token.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            service::sessions::refresh(&state, &token, ClientContext::default()).await
        })
    };
    let right = {
        let barrier = barrier.clone();
        let token = initial.refresh_token.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            service::sessions::refresh(&replica, &token, ClientContext::default()).await
        })
    };
    barrier.wait().await;
    let outcomes = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|r| matches!(r, Err(AccountError::RefreshReused)))
            .count(),
        1
    );
    assert!(
        service::sessions::authenticate(&f.state, &initial.access_token)
            .await
            .is_err()
    );
}

/// Holds an Account writer lock to deterministically exhaust a one-request coordinator and cancel its waiter.
/// Checks permit/lock reclamation, then same-instance contention with the production budget.
#[tokio::test]
async fn same_instance_contention_and_budget_are_bounded_and_reclaimable() {
    use sea_orm::TransactionTrait;
    let mut f = Fixture::new().await;
    let initial = pair(&f).await;
    let mut state = (*f.state).clone();
    let mut security = (*state.account_security).clone();
    security.sessions.coordinator = aster_forge_auth::RefreshCoordinator::new(1).unwrap();
    state.account_security = Arc::new(security);
    f.state = Arc::new(state);
    let coordinator = &f.state.account_security.sessions.coordinator;
    let txn = f.state.db_handles.writer().begin().await.unwrap();
    aster_gate::db::repository::session_repo::lock_account(
        &txn,
        &service::models::AccountId::parse(&f.initial_account_id).unwrap(),
    )
    .await
    .unwrap();
    let running = {
        let state = f.state.clone();
        let token = initial.refresh_token.clone();
        tokio::spawn(async move {
            service::sessions::refresh(&state, &token, ClientContext::default()).await
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while coordinator.active_requests() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        refresh(&f, &initial.refresh_token).await,
        Err(AccountError::RateLimited)
    ));
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    txn.rollback().await.unwrap();
    assert_eq!(coordinator.active_requests(), 0);
    assert_eq!(coordinator.tracked_ids(), 0);
    // Restore the production budget; both same-JTI callers must be admitted and serialized.
    let mut state = (*f.state).clone();
    let mut security = (*state.account_security).clone();
    security.sessions.coordinator = aster_forge_auth::RefreshCoordinator::default();
    state.account_security = Arc::new(security);
    f.state = Arc::new(state);
    let (left, right) = tokio::join!(
        refresh(&f, &initial.refresh_token),
        refresh(&f, &initial.refresh_token)
    );
    let results = [left, right];
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(AccountError::RefreshReused)))
            .count(),
        1
    );
    assert_eq!(
        f.state
            .account_security
            .sessions
            .coordinator
            .active_requests(),
        0
    );
    assert_eq!(
        f.state.account_security.sessions.coordinator.tracked_ids(),
        0
    );
}

/// Recreates runtime state after two rotations to test durable old-generation replay detection.
/// Changed key/issuer reject without compromise side effects; the original runtime observes committed reuse.
#[tokio::test]
async fn restart_rejects_older_generation_and_key_or_issuer_changes_invalidate_old_tokens() {
    let f = Fixture::new().await;
    let initial = pair(&f).await;
    let second = refresh(&f, &initial.refresh_token).await.unwrap();
    let third = refresh(&f, &second.refresh_token).await.unwrap();
    let replica = aster_gate::runtime::assembly::prepare_state((*f.state.config).clone())
        .await
        .unwrap();
    assert!(
        service::sessions::authenticate(&replica, &third.access_token)
            .await
            .is_ok()
    );
    let mut wrong_key = (*f.state.config).clone();
    wrong_key.session.signing_key = hex::encode([9_u8; 32]);
    let mut wrong_issuer = (*f.state.config).clone();
    wrong_issuer.session.issuer = "https://different.example.test".into();
    for config in [wrong_key, wrong_issuer] {
        let changed = aster_gate::runtime::assembly::prepare_state(config)
            .await
            .unwrap();
        assert!(
            service::sessions::authenticate(&changed, &third.access_token)
                .await
                .is_err()
        );
        assert!(matches!(
            service::sessions::refresh(&changed, &third.refresh_token, ClientContext::default())
                .await,
            Err(AccountError::Unauthenticated)
        ));
    }
    assert!(matches!(
        service::sessions::refresh(&replica, &initial.refresh_token, ClientContext::default())
            .await,
        Err(AccountError::RefreshReused)
    ));
    assert!(
        service::sessions::authenticate(&f.state, &third.access_token)
            .await
            .is_err()
    );
}

/// Uses an invalid database URL to prove signing policy and key-separation validation happen first.
#[tokio::test]
async fn invalid_signing_configuration_fails_before_database_startup() {
    for (issuer, key) in [
        ("http://public.example.test", hex::encode([8_u8; 32])),
        (
            "https://user:password@example.test",
            hex::encode([8_u8; 32]),
        ),
        ("https://example.test?issuer=other", hex::encode([8_u8; 32])),
        ("https://example.test", "broken".into()),
        ("https://example.test", hex::encode([7_u8; 32])),
    ] {
        let mut config = aster_gate::config::AppConfig::default();
        config.security.master_key = hex::encode([7_u8; 32]);
        config.session.issuer = issuer.into();
        config.session.signing_key = key.clone();
        config.database.url = "not-a-database-url".into();
        let error = match aster_gate::runtime::assembly::prepare_state(config).await {
            Ok(_) => panic!("invalid signing configuration accepted"),
            Err(error) => error,
        };
        assert!(matches!(error, aster_gate::errors::AppError::Config(_)));
        assert!(!format!("{error:?}").contains(&key));
    }
}

/// Injects audit insertion failure during reuse and checks that epoch and session authority remain unchanged.
#[tokio::test]
async fn audit_failure_rolls_back_reuse_epoch_and_revocation() {
    let f = Fixture::new().await;
    let initial = pair(&f).await;
    let next = refresh(&f, &initial.refresh_token).await.unwrap();
    f.reject_inserts("audit_logs").await;
    let response = request(
        &f,
        "/api/v1/auth/refresh",
        &format!("ag_refresh={}", initial.refresh_token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key("set-cookie"));
    assert!(
        service::sessions::authenticate(&f.state, &next.access_token)
            .await
            .is_ok()
    );
    assert!(refresh(&f, &next.refresh_token).await.is_ok());
}

/// Advances past access expiry and verifies current-refresh logout works while a predecessor cannot revoke it.
#[tokio::test]
async fn logout_uses_current_refresh_after_access_expiry_and_rejects_predecessor() {
    let mut f = Fixture::new().await;
    let initial = pair(&f).await;
    let next = refresh(&f, &initial.refresh_token).await.unwrap();
    let old = request(
        &f,
        "/api/v1/auth/logout",
        &format!("ag_refresh={}", initial.refresh_token),
    )
    .await;
    assert_eq!(old.status(), StatusCode::UNAUTHORIZED);
    assert!(!old.headers().contains_key("set-cookie"));
    clock(&mut f, next.access_expires_at);
    let response = request(
        &f,
        "/api/v1/auth/logout",
        &format!("ag_refresh={}", next.refresh_token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let values = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 3);
    assert!(values.iter().all(|v| v.contains("Max-Age=0")));
    assert!(
        values
            .iter()
            .any(|v| v.starts_with("ag_refresh=") && v.contains("Path=/api/v1/auth"))
    );
    assert!(refresh(&f, &next.refresh_token).await.is_err());
}

/// Exercises missing CSRF, duplicate credentials and access-as-refresh through the assembled router.
#[tokio::test]
async fn refresh_router_requires_csrf_and_rejects_cookie_ambiguity_and_wrong_purpose() {
    let f = Fixture::new().await;
    let pair = pair(&f).await;
    for cookies in [
        format!("ag_refresh={}", pair.access_token),
        format!("ag_refresh={0}; ag_refresh={0}", pair.refresh_token),
        "ag_refresh=bad".into(),
        "ag_refresh=".into(),
    ] {
        let response = request(&f, "/api/v1/auth/refresh", &cookies).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key("set-cookie"));
    }
    let response = f
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("cookie", format!("ag_refresh={}", pair.refresh_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(refresh(&f, &pair.refresh_token).await.is_ok());
}

/// Signs mismatched owner/sid, epoch, restriction and namespace claims with the trusted test key.
/// Each must reject without revoking legitimate sessions, including another existing Account.
#[tokio::test]
async fn signed_foreign_session_restriction_version_and_namespace_fail_without_reuse_effects() {
    let f = Fixture::new().await;
    let (other_id, other_cookie) = f.activate("foreign-session").await;
    let initial = pair(&f).await;
    let parsed = claims(&f, &initial.refresh_token, AuthTokenType::Refresh);
    let key = JwtKey::hmac(hex::decode(&f.state.config.session.signing_key).unwrap()).unwrap();
    for (subject, sid, epoch, restriction, issuer, audience) in [
        (
            other_id,
            parsed.sid.as_str().to_string(),
            0,
            Restriction::None,
            f.state.config.session.issuer.as_str(),
            "aster-gate-web-refresh",
        ),
        (
            uuid::Uuid::new_v4().to_string(),
            parsed.sid.as_str().to_string(),
            0,
            Restriction::None,
            f.state.config.session.issuer.as_str(),
            "aster-gate-web-refresh",
        ),
        (
            f.initial_account_id.clone(),
            uuid::Uuid::new_v4().to_string(),
            0,
            Restriction::None,
            f.state.config.session.issuer.as_str(),
            "aster-gate-web-refresh",
        ),
        (
            f.initial_account_id.clone(),
            parsed.sid.as_str().to_string(),
            1,
            Restriction::None,
            f.state.config.session.issuer.as_str(),
            "aster-gate-web-refresh",
        ),
        (
            f.initial_account_id.clone(),
            parsed.sid.as_str().to_string(),
            0,
            Restriction::MustChangePassword,
            f.state.config.session.issuer.as_str(),
            "aster-gate-web-refresh",
        ),
        (
            f.initial_account_id.clone(),
            parsed.sid.as_str().to_string(),
            0,
            Restriction::None,
            "https://foreign.example",
            "aster-gate-web-refresh",
        ),
        (
            f.initial_account_id.clone(),
            parsed.sid.as_str().to_string(),
            0,
            Restriction::None,
            f.state.config.session.issuer.as_str(),
            "aster-drive-web",
        ),
    ] {
        let forged = JwtSigner::with_system_clock(
            key.clone(),
            JwtPolicy::new(issuer, audience, 300, 86400).unwrap(),
        )
        .issue(
            Subject::new(subject).unwrap(),
            aster_forge_auth::SessionId::new(sid).unwrap(),
            SessionVersion::new(epoch),
            restriction,
        )
        .unwrap();
        assert!(refresh(&f, &forged.refresh_token).await.is_err());
        assert!(
            service::sessions::authenticate(&f.state, &initial.access_token)
                .await
                .is_ok()
        );
    }
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&other_cookie), false)
            .await
            .0,
        StatusCode::OK
    );
}

/// Seeds the maximum stored epoch to verify overflow rejection rolls back revocation.
/// Then shortens the server deadline to prove signed expiry cannot override writer authority.
#[tokio::test]
async fn writer_session_deadline_and_security_epoch_exhaustion_fail_closed() {
    use sea_orm::{ActiveModelTrait, IntoActiveModel, Set};
    let f = Fixture::new().await;
    let row = account::Entity::find_by_id(&f.initial_account_id)
        .one(f.state.db_handles.writer())
        .await
        .unwrap()
        .unwrap();
    let mut row = row.into_active_model();
    row.session_version = Set(i64::MAX);
    row.update(f.state.db_handles.writer()).await.unwrap();
    let initial = pair(&f).await;
    let principal = service::sessions::authenticate(&f.state, &initial.access_token)
        .await
        .unwrap();
    assert!(matches!(
        service::revoke_all_sessions(&f.state, &principal).await,
        Err(AccountError::Storage)
    ));
    // Failed epoch advancement must roll back rather than wrapping and revoking only half the state.
    assert!(
        service::sessions::authenticate(&f.state, &initial.access_token)
            .await
            .is_ok()
    );
    let parsed = claims(&f, &initial.refresh_token, AuthTokenType::Refresh);
    let row = session::Entity::find_by_id(parsed.sid.as_str())
        .one(f.state.db_handles.writer())
        .await
        .unwrap()
        .unwrap();
    let mut row = row.into_active_model();
    row.expires_at = Set(parsed.iat);
    row.update(f.state.db_handles.writer()).await.unwrap();
    assert!(
        service::sessions::authenticate(&f.state, &initial.access_token)
            .await
            .is_err()
    );
    assert!(matches!(
        refresh(&f, &initial.refresh_token).await,
        Err(AccountError::Unauthenticated)
    ));
}

/// Removes a JWT column from SQLite and checks startup reports incompatibility while preserving Account IDs.
#[tokio::test]
async fn incompatible_development_schema_fails_startup_without_resetting_data() {
    use sea_orm::QuerySelect;
    let f = Fixture::new().await;
    let db = f.state.db_handles.writer();
    if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
        return;
    }
    db.execute_unprepared("ALTER TABLE accounts DROP COLUMN session_version")
        .await
        .unwrap();
    let error = match aster_gate::runtime::assembly::prepare_state((*f.state.config).clone()).await
    {
        Ok(_) => panic!("old schema passed startup"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("pre-JWT development databases"));
    assert_eq!(
        account::Entity::find()
            .select_only()
            .column(account::Column::Id)
            .into_tuple::<String>()
            .all(db)
            .await
            .unwrap(),
        vec![f.initial_account_id]
    );
}

/// Uses SQLite triggers to force a lost CAS and deferred commit failure through the real refresh router.
/// Both must withhold candidate Cookies and leave the previous credential recoverable.
#[tokio::test]
async fn sqlite_cas_and_commit_failure_never_release_candidate_cookies() {
    let f = Fixture::new().await;
    let db = f.state.db_handles.writer();
    if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
        return;
    }
    let pair = pair(&f).await;
    db.execute_unprepared("CREATE TRIGGER ignore_rotation BEFORE UPDATE OF current_refresh_jti ON browser_sessions BEGIN SELECT RAISE(IGNORE); END").await.unwrap();
    let response = request(
        &f,
        "/api/v1/auth/refresh",
        &format!("ag_refresh={}", pair.refresh_token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key("set-cookie"));
    db.execute_unprepared("DROP TRIGGER ignore_rotation")
        .await
        .unwrap();
    db.execute_unprepared("CREATE TABLE commit_fence (account_id TEXT REFERENCES accounts(id) DEFERRABLE INITIALLY DEFERRED)").await.unwrap();
    db.execute_unprepared("CREATE TRIGGER fail_rotation_commit AFTER UPDATE OF current_refresh_jti ON browser_sessions BEGIN INSERT INTO commit_fence VALUES ('missing-account'); END").await.unwrap();
    let response = request(
        &f,
        "/api/v1/auth/refresh",
        &format!("ag_refresh={}", pair.refresh_token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key("set-cookie"));
    db.execute_unprepared("DROP TRIGGER fail_rotation_commit")
        .await
        .unwrap();
    assert!(refresh(&f, &pair.refresh_token).await.is_ok());
}

/// Closes one coordinator and proves refresh admission stops without revoking sessions or closing another instance.
#[tokio::test]
async fn coordinator_shutdown_is_instance_local_and_does_not_revoke_sessions() {
    let f = Fixture::new().await;
    let initial = pair(&f).await;
    f.state.account_security.sessions.coordinator.close();
    assert!(matches!(
        refresh(&f, &initial.refresh_token).await,
        Err(AccountError::Storage)
    ));
    assert!(
        service::sessions::authenticate(&f.state, &initial.access_token)
            .await
            .is_ok()
    );
    let other = Fixture::new().await;
    assert!(
        refresh(&other, &pair(&other).await.refresh_token)
            .await
            .is_ok()
    );
}

/// Signs malformed claims with raw JWT encoding to test Gate validation independently of Forge issuance.
/// Covers required fields, purpose/JTI rules, time/numeric bounds, wrong keys/algorithms and token size.
#[tokio::test]
async fn gate_pins_jwt_claims_algorithm_key_size_purpose_and_time_boundaries() {
    let mut f = Fixture::new().await;
    let initial = pair(&f).await;
    let parsed = claims(&f, &initial.access_token, AuthTokenType::Access);
    clock(&mut f, parsed.iat);
    let base = serde_json::to_value(&parsed).unwrap();
    let key = jsonwebtoken::EncodingKey::from_secret(
        &hex::decode(&f.state.config.session.signing_key).unwrap(),
    );
    let encode = |value: &Value, algorithm| {
        jsonwebtoken::encode(&jsonwebtoken::Header::new(algorithm), value, &key).unwrap()
    };
    for field in [
        "iss",
        "aud",
        "sub",
        "sid",
        "iat",
        "exp",
        "session_version",
        "token_type",
        "restriction",
    ] {
        let mut value = base.clone();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            matches!(
                service::sessions::authenticate(
                    &f.state,
                    &encode(&value, jsonwebtoken::Algorithm::HS256)
                )
                .await,
                Err(AccountError::Unauthenticated)
            ),
            "missing {field}"
        );
    }
    for (field, replacement) in [
        ("iss", json!("https://other.example")),
        ("aud", json!("aster-gate-web-refresh")),
        ("aud", json!(["aster-gate-web-access", "extra"])),
        ("sub", json!("not-a-uuid")),
        ("sid", json!("bad-session")),
        ("session_version", json!(u64::MAX)),
        ("session_version", json!(-1)),
        ("restriction", json!("unknown")),
        ("restriction", json!("must_change_password")),
        ("token_type", json!("refresh")),
        ("jti", json!("access-must-not-have-jti")),
        ("iat", json!(parsed.iat + 1)),
        ("nbf", json!(parsed.iat + 1)),
        ("exp", json!(parsed.iat)),
        ("exp", json!(u64::MAX)),
        ("iat", json!(-1)),
    ] {
        let mut value = base.clone();
        value[field] = replacement;
        assert!(
            matches!(
                service::sessions::authenticate(
                    &f.state,
                    &encode(&value, jsonwebtoken::Algorithm::HS256)
                )
                .await,
                Err(AccountError::Unauthenticated | AccountError::AccessExpired)
            ),
            "invalid {field}"
        );
    }
    for algorithm in [
        jsonwebtoken::Algorithm::HS384,
        jsonwebtoken::Algorithm::HS512,
    ] {
        assert!(matches!(
            service::sessions::authenticate(&f.state, &encode(&base, algorithm)).await,
            Err(AccountError::Unauthenticated)
        ));
    }
    let wrong_key = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &base,
        &jsonwebtoken::EncodingKey::from_secret(&[9_u8; 32]),
    )
    .unwrap();
    for token in [
        wrong_key,
        "a".repeat(20_000),
        "".into(),
        "a.b.c".into(),
        "a.b.c.d".into(),
    ] {
        assert!(matches!(
            service::sessions::authenticate(&f.state, &token).await,
            Err(AccountError::Unauthenticated)
        ));
    }
    let mut refresh_claims =
        serde_json::to_value(claims(&f, &initial.refresh_token, AuthTokenType::Refresh)).unwrap();
    refresh_claims.as_object_mut().unwrap().remove("jti");
    assert!(matches!(
        refresh(&f, &encode(&refresh_claims, jsonwebtoken::Algorithm::HS256)).await,
        Err(AccountError::Unauthenticated)
    ));
    assert!(
        service::sessions::authenticate(&f.state, &initial.access_token)
            .await
            .is_ok()
    );
}

/// Occupies SQLite's writer, waits for observable refresh progress, then advances time past expiry.
/// Also cancels a blocked refresh and checks coordinator guards are reclaimed before a later success.
#[tokio::test]
async fn writer_wait_rechecks_expiry_and_cancelled_wait_releases_instance_budget() {
    use sea_orm::TransactionTrait;
    let mut f = Fixture::new().await;
    let db = f.state.db_handles.writer().clone();
    // Deterministically occupying SQLite's single writer gives a real database wait.
    if db.get_database_backend() != sea_orm::DatabaseBackend::Sqlite {
        return;
    }
    let initial = pair(&f).await;
    let parsed = claims(&f, &initial.refresh_token, AuthTokenType::Refresh);
    let time = clock(&mut f, parsed.exp - 1);
    let baseline = time.1.load(Ordering::SeqCst);
    let txn = db.begin().await.unwrap();
    let running = {
        let state = f.state.clone();
        let token = initial.refresh_token.clone();
        tokio::spawn(async move {
            service::sessions::refresh(&state, &token, ClientContext::default()).await
        })
    };
    let coordinator = &f.state.account_security.sessions.coordinator;
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while coordinator.active_requests() != 1 || time.1.load(Ordering::SeqCst) < baseline + 5 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    time.0.store(parsed.exp, Ordering::SeqCst);
    txn.rollback().await.unwrap();
    assert!(matches!(
        running.await.unwrap(),
        Err(AccountError::Unauthenticated)
    ));
    assert_eq!(coordinator.active_requests(), 0);
    assert_eq!(coordinator.tracked_ids(), 0);

    time.0.store(parsed.iat, Ordering::SeqCst);
    let txn = db.begin().await.unwrap();
    let running = {
        let state = f.state.clone();
        let token = initial.refresh_token.clone();
        tokio::spawn(async move {
            service::sessions::refresh(&state, &token, ClientContext::default()).await
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while coordinator.active_requests() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    txn.rollback().await.unwrap();
    assert_eq!(coordinator.active_requests(), 0);
    assert_eq!(coordinator.tracked_ids(), 0);
    assert!(refresh(&f, &initial.refresh_token).await.is_ok());
}
