use super::{AccountError, Result, models::*, now, security};
use crate::services::authorization::{self, models::SystemPermission};
use crate::{
    db::{
        entities::account,
        repository::{account_repo as repo, account_writes as writes},
    },
    runtime::AppState,
};
use sea_orm::TransactionTrait;

pub async fn register(state: &AppState, command: RegisterAccount) -> Result<Account> {
    crate::services::system_setup::require_ready(state).await?;
    if state.runtime_config.authentication()?.registration
        != crate::services::system_settings::policy::RegistrationPolicy::Open
    {
        return Err(AccountError::Forbidden);
    }
    provision_inner(state, command, AuditAction::Registered, None).await
}

pub async fn provision(
    state: &AppState,
    operator: &Principal,
    command: RegisterAccount,
) -> Result<Account> {
    provision_inner(state, command, AuditAction::Provisioned, Some(operator)).await
}

async fn provision_inner(
    state: &AppState,
    command: RegisterAccount,
    action: AuditAction,
    operator: Option<&Principal>,
) -> Result<Account> {
    let permit = match operator {
        Some(principal) => Some(
            authorization::prepare_action(state, principal, SystemPermission::AccountsCreate, None)
                .await?,
        ),
        None => None,
    };
    let username = command.username.as_str().to_string();
    let email = command.email.as_str().to_string();
    let display_name = command.display_name.as_str().to_string();
    crate::services::system_setup::require_ready(state).await?;
    security::reserve_attempt(state, AuthenticationOperation::Register, &email, 5).await?;
    security::reserve_attempt(
        state,
        AuthenticationOperation::RegisterGlobal,
        "global",
        100,
    )
    .await?;
    let hash = state
        .account_security
        .hash(command.password.into_secret())
        .await?;
    let clock = now();
    let id = AccountId::new();
    let txn = state.db_handles.writer().begin().await?;
    crate::db::repository::system_config_repo::lock(&txn).await?;
    if let Some(permit) = permit {
        permit.recheck(&txn).await?;
    }
    let settings = crate::db::repository::system_config_repo::read_authentication(&txn).await?;
    if matches!(action, AuditAction::Registered)
        && settings.registration
            != crate::services::system_settings::policy::RegistrationPolicy::Open
    {
        return Err(AccountError::Forbidden);
    }
    writes::create_pending(
        &txn,
        &id,
        writes::NewAccountRecord {
            display_name,
            username,
            email: email.clone(),
            password_hash: hash,
        },
        clock,
    )
    .await?;
    if settings.email_verification
        == crate::services::system_settings::policy::EmailVerificationPolicy::Optional
    {
        writes::update_status(&txn, &id, AccountStatus::Active, None).await?;
    }
    if settings.mail_enabled {
        super::challenges::enqueue(state, &txn, &id, ChallengePurpose::VerifyEmail, &email).await?;
    }
    audit(
        &txn,
        &id,
        action,
        if matches!(action, AuditAction::Provisioned) {
            "account_operator"
        } else {
            "self"
        },
    )
    .await?;
    let row = repo::find(&txn, &id).await?.ok_or(AccountError::Storage)?;
    let result = project(&txn, &row).await?;
    txn.commit().await?;
    Ok(result)
}

pub async fn login(
    state: &AppState,
    input: &str,
    password: String,
    user_agent: &str,
) -> Result<LoginResult> {
    crate::services::system_setup::require_ready(state).await?;
    let session_lifetime = state
        .runtime_config
        .authentication()?
        .session_lifetime
        .seconds();
    let (kind, value) =
        security::login_identifier(input).map_err(|_| AccountError::InvalidCredentials)?;
    if password.len() > 128 {
        return Err(AccountError::InvalidCredentials);
    }
    security::reserve_attempt(state, AuthenticationOperation::Login, &value, 20).await?;
    let db = state.db_handles.writer();
    let row = repo::by_identifier(db, kind, &value).await?;
    security::reserve_attempt(state, AuthenticationOperation::LoginGlobal, "global", 1000).await?;
    if let Some(row) = &row {
        security::reserve_attempt(state, AuthenticationOperation::LoginAccount, &row.id, 20)
            .await?;
    }
    let stored = match &row {
        Some(row) => repo::password(db, &AccountId(row.id.clone())).await?,
        None => None,
    };
    let valid = state
        .account_security
        .verify(password, stored.map(|c| c.password_hash))
        .await?;
    let Some(row) = row.filter(|r| valid && AccountStatus::from(r.status) == AccountStatus::Active)
    else {
        return Err(AccountError::InvalidCredentials);
    };
    let txn = db.begin().await?;
    if !repo::claim_revision(&txn, &AccountId(row.id.clone()), row.revision, now()).await? {
        return Err(AccountError::InvalidCredentials);
    }
    let id = AccountId(row.id.clone());
    if repo::sessions(&txn, &id, now()).await?.len() >= 20 {
        return Err(AccountError::RateLimited);
    }
    let token = security::random_token();
    writes::create_session(
        &txn,
        &id,
        security::digest(&token),
        user_agent.chars().take(128).collect(),
        now(),
        now() + session_lifetime,
    )
    .await?;
    audit(&txn, &id, AuditAction::SignedIn, "self").await?;
    let account = project(&txn, &row).await?;
    txn.commit().await?;
    Ok(LoginResult {
        account,
        session_token: token,
    })
}

pub async fn authenticate(state: &AppState, token: &str) -> Result<Principal> {
    if token.len() != 64 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(AccountError::Unauthenticated);
    }
    let db = state.db_handles.writer();
    let session = repo::session_by_token(db, &security::digest(token), now())
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    let account = repo::find(db, &AccountId(session.account_id.clone()))
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    if AccountStatus::from(account.status) != AccountStatus::Active {
        return Err(AccountError::Unauthenticated);
    }
    Ok(Principal {
        account_id: AccountId(account.id),
        revision: account.revision,
        session_id: SessionId(session.id),
        token_hash: session.token_hash,
    })
}

pub async fn profile(state: &AppState, principal: &Principal) -> Result<Account> {
    let row = repo::find(state.db_handles.writer(), &principal.account_id)
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    project(state.db_handles.writer(), &row).await
}

pub async fn update_profile(
    state: &AppState,
    principal: &Principal,
    name: &str,
) -> Result<Account> {
    let name = security::display_name(name)?;
    let txn = security::principal_transaction(state, principal).await?;
    let id = principal.account_id.clone();
    writes::update_profile(&txn, &id, name).await?;
    audit(&txn, &id, AuditAction::ProfileUpdated, "self").await?;
    let row = repo::find(&txn, &id).await?.ok_or(AccountError::NotFound)?;
    let result = project(&txn, &row).await?;
    txn.commit().await?;
    Ok(result)
}

pub async fn list_sessions(state: &AppState, principal: &Principal) -> Result<Vec<Session>> {
    Ok(repo::sessions(
        state.db_handles.writer(),
        &AccountId(principal.account_id.0.clone()),
        now(),
    )
    .await?
    .into_iter()
    .map(|s| Session {
        current: s.id == principal.session_id.0,
        id: SessionId(s.id),
        created_at: s.created_at,
        expires_at: s.expires_at,
        user_agent: s.user_agent,
    })
    .collect())
}

pub async fn revoke_session(state: &AppState, principal: &Principal, id: &SessionId) -> Result<()> {
    let txn = security::principal_transaction(state, principal).await?;
    let account = AccountId(principal.account_id.0.clone());
    if !repo::revoke_session(&txn, &account, id, now()).await? {
        return Err(AccountError::NotFound);
    }
    audit(
        &txn,
        &account,
        if id.0 == principal.session_id.0 {
            AuditAction::SignedOut
        } else {
            AuditAction::SessionRevoked
        },
        "self",
    )
    .await?;
    txn.commit().await?;
    Ok(())
}

pub async fn revoke_all_sessions(state: &AppState, principal: &Principal) -> Result<()> {
    let txn = security::principal_transaction(state, principal).await?;
    let id = AccountId(principal.account_id.0.clone());
    repo::revoke_sessions(&txn, &id, now()).await?;
    audit(&txn, &id, AuditAction::SessionsRevoked, "self").await?;
    txn.commit().await?;
    Ok(())
}

pub(crate) async fn prove_password(
    state: &AppState,
    principal: &Principal,
    password: String,
) -> Result<()> {
    security::reserve_attempt(
        state,
        AuthenticationOperation::StepUp,
        &principal.account_id.0,
        10,
    )
    .await?;
    if password.len() > 128 {
        return Err(AccountError::InvalidCredentials);
    }
    let credential = repo::password(
        state.db_handles.writer(),
        &AccountId(principal.account_id.0.clone()),
    )
    .await?
    .ok_or(AccountError::InvalidCredentials)?;
    if !state
        .account_security
        .verify(password, Some(credential.password_hash))
        .await?
    {
        return Err(AccountError::InvalidCredentials);
    }
    Ok(())
}

pub async fn change_password(
    state: &AppState,
    principal: &Principal,
    current: String,
    new: String,
) -> Result<()> {
    security::validate_password(&new)?;
    if current == new {
        return Err(AccountError::InvalidInput);
    }
    prove_password(state, principal, current).await?;
    let hash = state.account_security.hash(new).await?;
    let txn = security::principal_transaction(state, principal).await?;
    let id = AccountId(principal.account_id.0.clone());
    set_password(&txn, &id, hash).await?;
    super::challenges::cancel(&txn, &id, None).await?;
    audit(&txn, &id, AuditAction::PasswordChanged, "self").await?;
    txn.commit().await?;
    Ok(())
}

pub async fn delete_self(state: &AppState, principal: &Principal, password: String) -> Result<()> {
    prove_password(state, principal, password).await?;
    let txn = state.db_handles.writer().begin().await?;
    crate::db::repository::system_config_repo::lock(&txn).await?;
    authorization::protect_last_operator(&txn, &principal.account_id).await?;
    if repo::session_by_token(&txn, &principal.token_hash, now())
        .await?
        .is_none()
        || !repo::claim_revision(&txn, &principal.account_id, principal.revision, now()).await?
    {
        return Err(AccountError::Unauthenticated);
    }
    erase(&txn, &AccountId(principal.account_id.0.clone()), "self").await?;
    txn.commit().await?;
    Ok(())
}

pub async fn manage_lifecycle(
    state: &AppState,
    operator: &Principal,
    id: &AccountId,
    action: LifecycleAction,
    password: Option<String>,
) -> Result<Account> {
    let permission = match action {
        LifecycleAction::Suspend => SystemPermission::AccountsSuspend,
        LifecycleAction::Restore => SystemPermission::AccountsRestore,
        LifecycleAction::Delete => SystemPermission::AccountsDelete,
    };
    let permit = authorization::prepare_action(state, operator, permission, password).await?;
    let db = state.db_handles.writer();
    let txn = db.begin().await?;
    crate::db::repository::system_config_repo::lock(&txn).await?;
    permit.recheck(&txn).await?;
    let row = repo::find(&txn, id).await?.ok_or(AccountError::NotFound)?;
    let status: AccountStatus = row.status.into();
    if matches!(action, LifecycleAction::Suspend | LifecycleAction::Delete) {
        authorization::protect_last_operator(&txn, id).await?;
    }
    if !repo::claim_revision(&txn, &AccountId(row.id.clone()), row.revision, now()).await? {
        return Err(AccountError::Conflict);
    }
    match action {
        LifecycleAction::Suspend
            if matches!(
                status,
                AccountStatus::Active | AccountStatus::PendingVerification
            ) =>
        {
            writes::update_status(&txn, id, AccountStatus::Suspended, Some(status)).await?;
            repo::revoke_sessions(&txn, id, now()).await?;
            super::challenges::cancel(&txn, id, None).await?;
            audit(&txn, id, AuditAction::Suspended, "account_operator").await?;
        }
        LifecycleAction::Restore if status == AccountStatus::Suspended => {
            let target = row.suspended_from.ok_or(AccountError::InvalidTransition)?;
            if ![AccountStatus::Active, AccountStatus::PendingVerification].contains(&target.into())
            {
                return Err(AccountError::InvalidTransition);
            }
            writes::update_status(&txn, id, target.into(), None).await?;
            audit(&txn, id, AuditAction::Restored, "account_operator").await?;
        }
        LifecycleAction::Delete if status != AccountStatus::Deleted => {
            erase(&txn, id, "account_operator").await?;
        }
        _ => return Err(AccountError::InvalidTransition),
    }
    let row = repo::find(&txn, id).await?.ok_or(AccountError::NotFound)?;
    let result = project(&txn, &row).await?;
    txn.commit().await?;
    Ok(result)
}

pub async fn manage_list(
    state: &AppState,
    operator: &Principal,
    after: Option<&AccountId>,
    limit: u64,
) -> Result<Vec<Account>> {
    if !(1..=100).contains(&limit) {
        return Err(AccountError::InvalidInput);
    }
    let db = state.db_handles.writer();
    authorization::require(db, operator, SystemPermission::AccountsRead).await?;
    let mut results = Vec::new();
    for row in repo::list(db, after, limit).await? {
        results.push(project(db, &row).await?);
    }
    Ok(results)
}

pub(crate) async fn project<C: sea_orm::ConnectionTrait>(
    db: &C,
    row: &account::Model,
) -> Result<Account> {
    let id = AccountId(row.id.clone());
    let ids = repo::identifiers(db, &id).await?;
    let find = |kind: IdentifierKind| ids.iter().find(|i| IdentifierKind::from(i.kind) == kind);
    Ok(Account {
        id,
        username: find(IdentifierKind::Username)
            .map(|i| i.value.clone())
            .unwrap_or_default(),
        email: find(IdentifierKind::Email)
            .map(|i| i.value.clone())
            .unwrap_or_default(),
        email_verified: find(IdentifierKind::Email).is_some_and(|i| i.verified_at.is_some()),
        display_name: row.display_name.clone(),
        status: row.status.into(),
        created_at: row.created_at,
        updated_at: row.updated_at,
        system_permissions: crate::db::repository::system_permission_grant_repo::list(
            db,
            &AccountId(row.id.clone()),
        )
        .await?,
    })
}

pub(crate) async fn set_password(
    txn: &sea_orm::DatabaseTransaction,
    id: &AccountId,
    hash: String,
) -> Result<()> {
    writes::update_password(txn, id, hash, now()).await?;
    repo::revoke_sessions(txn, id, now()).await?;
    Ok(())
}

async fn erase(txn: &sea_orm::DatabaseTransaction, id: &AccountId, actor: &str) -> Result<()> {
    super::challenges::cancel(txn, id, None).await?;
    writes::erase_records(txn, id).await?;
    audit(txn, id, AuditAction::Deleted, actor).await?;
    Ok(())
}

pub(crate) async fn audit(
    txn: &sea_orm::DatabaseTransaction,
    id: &AccountId,
    action: AuditAction,
    actor: &str,
) -> Result<()> {
    // Forge's numeric actor column is reserved for system events. UUID Account references live in typed product details.
    aster_forge_db::audit_log::create_audit_log_row(
        txn,
        aster_forge_db::AuditLogCreate {
            user_id: 0,
            action: action.as_str().into(),
            entity_type: "account".into(),
            entity_id: None,
            entity_name: None,
            details: Some(
                serde_json::json!({"account_id": id.as_str(), "actor": actor}).to_string(),
            ),
            ip_address: None,
            user_agent: None,
            created_at: chrono::Utc::now(),
        },
    )
    .await
    .map_err(|_| AccountError::Storage)?;
    Ok(())
}
