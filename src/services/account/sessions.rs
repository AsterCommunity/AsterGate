//! Gate's first-party JWT adapter. Forge owns cryptography/refresh coordination; Gate owns
//! account authority, immutable session deadlines, writer transactions and security audit.
use super::{AccountError, Result, models::*};
use crate::{
    db::{
        entities::{account, browser_session},
        repository::{account_repo, session_repo},
    },
    runtime::AppState,
};
use aster_forge_auth::{
    AuthError, AuthTokenPair, AuthTokenType, ClientContext, Clock, FixedClock, JwtKey, JwtPolicy,
    JwtSigner, JwtVerifier, RandomIdSource, RefreshCoordinator, RefreshDecision, RefreshError,
    RefreshOutcome, RefreshPolicy, RefreshRejection, RefreshRotationRequest, RefreshSession,
    RefreshSessionStore, RefreshStoreOutcome, RefreshTokenService, Restriction, SessionVersion,
    Subject, TokenClaims, classify_refresh,
};
use sea_orm::{ConnectionTrait, DatabaseTransaction, TransactionTrait};
use std::sync::{Arc, OnceLock};

const ACCESS_AUDIENCE: &str = "aster-gate-web-access";
const REFRESH_AUDIENCE: &str = "aster-gate-web-refresh";
pub const ACCESS_TTL: i64 = 300;

#[derive(Clone)]
pub struct SessionRuntime {
    key: JwtKey,
    issuer: String,
    pub coordinator: RefreshCoordinator,
    clock: Arc<dyn Clock>,
}

impl SessionRuntime {
    /// Validates the deployment signing namespace and creates an instance-owned refresh coordinator.
    /// Only Forge crypto-provider installation is process-wide; invalid keys fail before database startup.
    pub(crate) fn new(config: &crate::config::AppConfig) -> crate::errors::Result<Self> {
        // Forge's provider is explicitly process-scoped; product state/coordinators remain isolated.
        static PROVIDER: OnceLock<aster_forge_auth::Result<()>> = OnceLock::new();
        PROVIDER
            .get_or_init(aster_forge_auth::initialize_jwt_crypto_provider)
            .map_err(|_| {
                crate::errors::AppError::Config("cannot initialize JWT crypto provider".into())
            })?;
        let key = config.session.key()?;
        if config
            .session
            .signing_key
            .eq_ignore_ascii_case(&config.security.master_key)
        {
            return Err(crate::errors::AppError::Config(
                "session signing key must be independent of security.master_key".into(),
            ));
        }
        JwtPolicy::new(&config.session.issuer, ACCESS_AUDIENCE, 300, 86400).map_err(|_| {
            crate::errors::AppError::Config("invalid first-party signing namespace".into())
        })?;
        Ok(Self {
            key,
            issuer: config.session.issuer.clone(),
            coordinator: RefreshCoordinator::default(),
            clock: Arc::new(aster_forge_auth::SystemClock),
        })
    }

    /// Clones this runtime with an injected clock while retaining its signing policy and coordinator.
    /// Account/session authority stays in the database; no security snapshot is cached here.
    pub fn with_clock(&self, clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            ..self.clone()
        }
    }

    /// Reads the injected trusted clock, reporting clock failures as security infrastructure errors.
    fn now(&self) -> Result<i64> {
        self.clock
            .now_unix_seconds()
            .map_err(|_| AccountError::Security)
    }

    /// Selects the exact credential audience with zero leeway and access capped at five minutes.
    /// Issuance passes the remaining absolute session lifetime; verification uses the configured bounds.
    fn policy(&self, purpose: AuthTokenType, ttl: u64) -> Result<JwtPolicy> {
        JwtPolicy::new(
            &self.issuer,
            match purpose {
                AuthTokenType::Access => ACCESS_AUDIENCE,
                AuthTokenType::Refresh => REFRESH_AUDIENCE,
            },
            ttl.min(300),
            ttl,
        )
        .and_then(|policy| policy.with_leeway(0))
        .map_err(|_| AccountError::Security)
    }

    /// Builds a purpose-specific verifier using the instance key and live clock.
    fn verifier(&self, purpose: AuthTokenType) -> Result<JwtVerifier> {
        Ok(JwtVerifier::new(
            self.key.clone(),
            self.policy(purpose, 86400)?,
            self.clock.clone(),
        ))
    }

    /// Pins issuance to a clock snapshot and caps both credentials at the absolute session deadline.
    /// An exhausted or overflowing lifetime rejects issuance before session insertion or rotation.
    fn signer(&self, purpose: AuthTokenType, now: i64, deadline: i64) -> Result<JwtSigner> {
        let remaining = deadline
            .checked_sub(now)
            .and_then(|value| u64::try_from(value).ok())
            .filter(|value| *value > 0)
            .ok_or(AccountError::Unauthenticated)?;
        // A stable issuance timestamp + remaining TTL preserves the signed absolute deadline,
        // including when the coordinator/store waits. Server expiry is independently authoritative.
        Ok(JwtSigner::new(
            self.key.clone(),
            self.policy(purpose, remaining)?,
            Arc::new(FixedClock::new(now)),
            Arc::new(RandomIdSource),
        ))
    }

    /// Combines credentials from distinct audience policies into one candidate pair.
    /// The caller must persist the refresh generation and commit before exposing either credential.
    fn issue(
        &self,
        subject: Subject,
        sid: aster_forge_auth::SessionId,
        version: SessionVersion,
        restriction: Restriction,
        now: i64,
        deadline: i64,
    ) -> Result<AuthTokenPair> {
        let access = self
            .signer(AuthTokenType::Access, now, deadline)?
            .issue(subject.clone(), sid.clone(), version, restriction)
            .map_err(|_| AccountError::Security)?;
        let mut pair = self
            .signer(AuthTokenType::Refresh, now, deadline)?
            .issue(subject, sid, version, restriction)
            .map_err(|_| AccountError::Security)?;
        pair.access_token = access.access_token;
        pair.access_expires_at = access.access_expires_at;
        Ok(pair)
    }

    /// Verifies Forge claims before converting UUID and epoch values to Gate storage boundaries.
    /// Expired access is refresh-eligible; invalid credentials and infrastructure failures remain distinct.
    fn verify(&self, token: &str, purpose: AuthTokenType) -> Result<TokenClaims> {
        let claims = self
            .verifier(purpose)?
            .verify(token, purpose)
            .map_err(|error| match error {
                AuthError::Expired if purpose == AuthTokenType::Access => {
                    AccountError::AccessExpired
                }
                AuthError::ResourceLimit => AccountError::RateLimited,
                AuthError::CryptoProviderNotInitialized
                | AuthError::CryptoProviderAlreadyInstalled
                | AuthError::KeyTooShort
                | AuthError::Encoding => AccountError::Security,
                _ => AccountError::Unauthenticated,
            })?
            .into_claims();
        account_id(&claims.sub)?;
        SessionId::parse(claims.sid.as_str()).map_err(|_| AccountError::Unauthenticated)?;
        stored_version(claims.session_version)?;
        Ok(claims)
    }
}

/// Requires a canonical Account UUID so alternate spellings cannot select a different identity key.
fn account_id(subject: &Subject) -> Result<AccountId> {
    let id = AccountId::parse(subject.as_str()).map_err(|_| AccountError::Unauthenticated)?;
    if id.as_str() != subject.as_str() {
        return Err(AccountError::Unauthenticated);
    }
    Ok(id)
}

/// Rejects signed epochs outside the database integer range before querying security state.
fn stored_version(version: SessionVersion) -> Result<i64> {
    i64::try_from(version.value()).map_err(|_| AccountError::Unauthenticated)
}

/// Converts a stored epoch to Forge claims; a negative stored value is a storage failure.
fn version(value: i64) -> Result<SessionVersion> {
    Ok(SessionVersion::new(
        u64::try_from(value).map_err(|_| AccountError::Storage)?,
    ))
}

/// Uses the caller's existing writer transaction: no nested pool acquisition or token delivery.
pub(crate) async fn create(
    state: &AppState,
    txn: &DatabaseTransaction,
    row: &account::Model,
    user_agent: &str,
    now: i64,
    expires_at: i64,
) -> Result<AuthTokenPair> {
    let sid = uuid::Uuid::new_v4().to_string();
    let pair = state.account_security.sessions.issue(
        Subject::new(row.id.clone()).map_err(|_| AccountError::Storage)?,
        aster_forge_auth::SessionId::new(&sid).map_err(|_| AccountError::Storage)?,
        version(row.session_version)?,
        Restriction::None,
        now,
        expires_at,
    )?;
    session_repo::create(
        txn,
        browser_session::Model {
            id: sid,
            account_id: row.id.clone(),
            session_version: row.session_version,
            current_refresh_jti: (&pair.refresh_jti).into(),
            previous_refresh_jti: None,
            last_rotated_at: now,
            user_agent: user_agent.chars().take(128).collect(),
            created_at: now,
            expires_at,
            revoked_at: None,
        },
    )
    .await?;
    Ok(pair)
}

/// Shared by request authentication and transaction-time authorization after account fencing.
pub(crate) async fn validate_principal<C: ConnectionTrait>(
    db: &C,
    principal: &Principal,
    now: i64,
) -> Result<account::Model> {
    let account = account_repo::find(db, &principal.account_id)
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    let session = session_repo::find(db, &principal.account_id, &principal.session_id)
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    if AccountStatus::from(account.status) != AccountStatus::Active
        || version(account.session_version)? != principal.session_version
        || session.session_version != account.session_version
        || session.revoked_at.is_some()
        || now >= session.expires_at
        || principal.restriction != Restriction::None
    {
        return Err(AccountError::Unauthenticated);
    }
    Ok(account)
}

/// Authenticates access against writer-authoritative Account and owned session state.
/// Returns the current mutation revision for later transaction fencing; claims do not grant permissions.
pub async fn authenticate(state: &AppState, token: &str) -> Result<Principal> {
    let runtime = &state.account_security.sessions;
    let claims = runtime.verify(token, AuthTokenType::Access)?;
    let id = account_id(&claims.sub)?;
    let db = state.db_handles.writer();
    let mut principal = Principal {
        account_id: id,
        session_id: SessionId::parse(claims.sid.as_str())?,
        revision: 0,
        session_version: claims.session_version,
        restriction: claims.restriction,
    };
    let row = validate_principal(db, &principal, runtime.now()?).await?;
    principal.revision = row.revision;
    Ok(principal)
}

/// Refresh-only entry point; protected APIs never accept refresh credentials.
pub async fn refresh(
    state: &AppState,
    token: &str,
    client: ClientContext,
) -> Result<AuthTokenPair> {
    let runtime = &state.account_security.sessions;
    let claims = runtime.verify(token, AuthTokenType::Refresh)?;
    let now = runtime.now()?;
    // Prepare the correctly scoped access credential before persistence; the shared refresh
    // service signs its candidate in the refresh namespace. Neither escapes on a store failure.
    let access = runtime
        .signer(AuthTokenType::Access, now, claims.exp)?
        .issue(
            claims.sub.clone(),
            claims.sid.clone(),
            claims.session_version,
            claims.restriction,
        )
        .map_err(|_| AccountError::Security)?;
    let service = RefreshTokenService::new(
        runtime.signer(AuthTokenType::Refresh, now, claims.exp)?,
        runtime.verifier(AuthTokenType::Refresh)?,
        Arc::new(GateRefreshStore {
            state: state.clone(),
        }),
        runtime.clock.clone(),
        RefreshPolicy::Strict,
        runtime.coordinator.clone(),
    );
    match service.refresh(token, client).await {
        Ok(RefreshOutcome::Rotated(mut pair)) => {
            pair.access_token = access.access_token;
            pair.access_expires_at = access.access_expires_at;
            Ok(pair)
        }
        Ok(RefreshOutcome::Rejected(reason)) => Err(rejection(reason)),
        Err(RefreshError::Store(error)) => Err(error),
        Err(RefreshError::Auth(AuthError::ResourceLimit)) => Err(AccountError::RateLimited),
        Err(RefreshError::Auth(AuthError::Cancelled)) => Err(AccountError::Storage),
        Err(RefreshError::Auth(_)) => Err(AccountError::Unauthenticated),
    }
}

/// Preserves typed stale/reuse results while mapping other refresh denials to session-required.
fn rejection(reason: RefreshRejection) -> AccountError {
    match reason {
        RefreshRejection::Stale => AccountError::RefreshStale,
        RefreshRejection::ReuseDetected => AccountError::RefreshReused,
        _ => AccountError::Unauthenticated,
    }
}

/// Logout accepts only the current refresh credential and works after access has expired.
/// A stale credential cannot revoke the session that replaced it.
pub async fn logout(state: &AppState, token: &str) -> Result<()> {
    let runtime = &state.account_security.sessions;
    let claims = runtime.verify(token, AuthTokenType::Refresh)?;
    let id = account_id(&claims.sub)?;
    let sid = SessionId::parse(claims.sid.as_str())?;
    let txn = state.db_handles.writer().begin().await?;
    let row = session_repo::lock_account(&txn, &id)
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    let principal = Principal {
        account_id: id.clone(),
        session_id: sid.clone(),
        revision: row.revision,
        session_version: claims.session_version,
        restriction: claims.restriction,
    };
    let now = runtime.now()?;
    validate_principal(&txn, &principal, now).await?;
    let session = session_repo::find(&txn, &id, &sid)
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    if now >= claims.exp
        || claims
            .jti
            .as_ref()
            .is_none_or(|jti| jti.as_str() != session.current_refresh_jti.as_str())
    {
        return Err(AccountError::Unauthenticated);
    }
    if !account_repo::revoke_session(&txn, &id, &sid, now).await? {
        return Err(AccountError::Unauthenticated);
    }
    super::lifecycle::audit(&txn, &id, AuditAction::SignedOut, "self").await?;
    txn.commit().await?;
    Ok(())
}

struct GateRefreshStore {
    state: AppState,
}

impl GateRefreshStore {
    /// Locks the Account in the caller transaction, then refreshes time before checking expiry.
    /// Checks Account epoch/restriction, then lets Forge classify the owned session's liveness and JTI.
    async fn decide(
        &self,
        txn: &DatabaseTransaction,
        original: &RefreshRotationRequest,
    ) -> Result<(RefreshDecision, RefreshRotationRequest)> {
        let id = account_id(&original.subject)?;
        let row = session_repo::lock_account(txn, &id).await?;
        let request = RefreshRotationRequest {
            now: original
                .now
                .max(self.state.account_security.sessions.now()?),
            ..original.clone()
        };
        let reject = |reason| (RefreshDecision::Reject(reason), request.clone());
        if i128::from(request.now) >= request.current_refresh_valid_until {
            return Ok(reject(RefreshRejection::Expired));
        }
        let Some(row) = row else {
            return Ok(reject(RefreshRejection::Unavailable));
        };
        if AccountStatus::from(row.status) != AccountStatus::Active {
            return Ok(reject(RefreshRejection::AccountDisabled));
        }
        if version(row.session_version)? != request.session_version {
            return Ok(reject(RefreshRejection::Unavailable));
        }
        if request.restriction != Restriction::None {
            return Ok(reject(RefreshRejection::Restricted));
        }
        let session =
            session_repo::find(txn, &id, &SessionId::parse(request.session_id.as_str())?).await?;
        if session
            .as_ref()
            .is_some_and(|session| session.session_version != row.session_version)
        {
            return Ok(reject(RefreshRejection::Unavailable));
        }
        let snapshot = session
            .as_ref()
            .map(|session| -> Result<RefreshSession> {
                let corrupt = |_| AccountError::Storage;
                Ok(RefreshSession {
                    subject: Subject::new(&session.account_id).map_err(corrupt)?,
                    session_id: aster_forge_auth::SessionId::new(&session.id).map_err(corrupt)?,
                    current_refresh_jti: aster_forge_auth::RefreshJti::new(
                        session.current_refresh_jti.as_str(),
                    )
                    .map_err(corrupt)?,
                    previous_refresh_jti: session
                        .previous_refresh_jti
                        .as_ref()
                        .map(|jti| aster_forge_auth::RefreshJti::new(jti.as_str()))
                        .transpose()
                        .map_err(corrupt)?,
                    last_seen_at: session.last_rotated_at,
                    refresh_expires_at: session.expires_at,
                    revoked: session.revoked_at.is_some(),
                    client: ClientContext {
                        user_agent: Some(session.user_agent.clone()),
                        ip_address: None,
                    },
                })
            })
            .transpose()?;
        Ok((classify_refresh(&request, snapshot.as_ref()), request))
    }

    /// Persists confirmed reuse revocation, epoch advancement and audit in the caller transaction.
    /// Returning a rejection does not commit it: the store must commit before delivering the denial.
    async fn reject(
        &self,
        txn: &DatabaseTransaction,
        request: &RefreshRotationRequest,
        reason: RefreshRejection,
    ) -> Result<RefreshRejection> {
        if reason == RefreshRejection::ReuseDetected {
            let id = account_id(&request.subject)?;
            account_repo::revoke_sessions(txn, &id, request.now).await?;
            super::lifecycle::audit(txn, &id, AuditAction::RefreshReused, "session_security")
                .await?;
        }
        Ok(reason)
    }
}

#[async_trait::async_trait]
impl RefreshSessionStore for GateRefreshStore {
    type Error = AccountError;

    /// Commits either a fenced JTI successor or the security effects of a classified rejection.
    /// CAS failure returns Conflict for a fresh transaction; storage or commit failure returns no credentials.
    async fn rotate_refresh(
        &self,
        request: &RefreshRotationRequest,
    ) -> Result<RefreshStoreOutcome> {
        let txn = self.state.db_handles.writer().begin().await?;
        let (decision, request) = self.decide(&txn, request).await?;
        let outcome = match decision {
            RefreshDecision::Rotate => {
                if session_repo::rotate(&txn, &request).await? {
                    RefreshStoreOutcome::Rotated
                } else {
                    RefreshStoreOutcome::Conflict
                }
            }
            RefreshDecision::Reject(reason) => {
                RefreshStoreOutcome::Rejected(self.reject(&txn, &request, reason).await?)
            }
        };
        txn.commit().await?;
        Ok(outcome)
    }

    /// Rereads a CAS loser in a new writer transaction and commits any confirmed reuse effects.
    /// A snapshot that still permits rotation is a storage conflict, never permission to rotate twice.
    async fn resolve_conflict(&self, request: &RefreshRotationRequest) -> Result<RefreshRejection> {
        let txn = self.state.db_handles.writer().begin().await?;
        let (decision, request) = self.decide(&txn, request).await?;
        let RefreshDecision::Reject(reason) = decision else {
            return Err(AccountError::Storage);
        };
        let reason = self.reject(&txn, &request, reason).await?;
        txn.commit().await?;
        Ok(reason)
    }
}
