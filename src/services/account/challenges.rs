use super::{AccountError, Result, models::*, now, security};
use crate::{
    db::{
        entities::account_challenge as challenge,
        repository::{
            account_repo as repo, account_writes as writes, challenge_repo as challenges,
        },
    },
    runtime::AppState,
};
use sea_orm::{ConnectionTrait, TransactionTrait};

pub(crate) const MAIL_CONTEXT: &[u8] = b"astergate.account-mail.v1";

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MailPayload {
    pub challenge_id: String,
    pub envelope: String,
}

pub(crate) async fn ensure_mail(state: &AppState) -> Result<()> {
    if state.runtime_config.authentication()?.mail_enabled {
        Ok(())
    } else {
        Err(AccountError::MailUnavailable)
    }
}

pub(crate) async fn enqueue(
    state: &AppState,
    txn: &sea_orm::DatabaseTransaction,
    id: &AccountId,
    purpose: ChallengePurpose,
    target: &str,
) -> Result<()> {
    let settings = crate::db::repository::system_config_repo::read_authentication(txn).await?;
    if !settings.mail_enabled {
        return Err(AccountError::MailUnavailable);
    }
    cancel(txn, id, Some(purpose)).await?;
    let clock = now();
    let token = security::random_token();
    let challenge_id = uuid::Uuid::new_v4().to_string();
    let key = &state.account_security.master_key;
    let envelope = aster_forge_crypto::encrypt_secret(
        key,
        MAIL_CONTEXT,
        challenge_id.as_bytes(),
        token.as_bytes(),
    )?;
    let code = match purpose {
        ChallengePurpose::VerifyEmail => aster_forge_mail::MailTemplateCode::RegisterActivation,
        ChallengePurpose::ResetPassword => aster_forge_mail::MailTemplateCode::PasswordReset,
        ChallengePurpose::ReplaceEmail => {
            aster_forge_mail::MailTemplateCode::ContactChangeConfirmation
        }
    };
    let payload = serde_json::to_string(&MailPayload {
        challenge_id: challenge_id.clone(),
        envelope,
    })
    .map_err(|_| AccountError::Security)?;
    let row = aster_forge_db::mail_outbox::create_mail_outbox_row(
        txn,
        aster_forge_db::MailOutboxCreate {
            template_code: code,
            to_address: target.into(),
            to_name: None,
            payload_json: payload.into(),
            now: chrono::Utc::now(),
            next_attempt_at: chrono::Utc::now(),
        },
    )
    .await
    .map_err(|_| AccountError::Storage)?;
    challenges::create(
        txn,
        challenges::NewChallengeRecord {
            id: challenge_id,
            account_id: id.clone(),
            purpose,
            token_hash: security::digest(&token),
            target_email: target.into(),
            created_at: clock,
            expires_at: clock + settings.challenge_lifetime.seconds(),
            outbox_id: row.id,
        },
    )
    .await?;
    Ok(())
}

pub(crate) async fn cancel(
    txn: &sea_orm::DatabaseTransaction,
    id: &AccountId,
    purpose: Option<ChallengePurpose>,
) -> Result<()> {
    for c in challenges::for_account(txn, id, purpose).await? {
        if let Some(outbox_id) = c.outbox_id {
            challenges::redact_and_cancel_mail(txn, outbox_id).await?;
        }
    }
    repo::invalidate_challenges(txn, id, purpose, now()).await?;
    Ok(())
}

pub async fn request_challenge(
    state: &AppState,
    input: &str,
    purpose: ChallengePurpose,
) -> Result<()> {
    if purpose == ChallengePurpose::ReplaceEmail {
        return Err(AccountError::Forbidden);
    }
    ensure_mail(state).await?;
    let (kind, value) = security::login_identifier(input)?;
    security::reserve_attempt(state, purpose.into(), &value, 3).await?;
    security::reserve_attempt(state, AuthenticationOperation::MailGlobal, "global", 100).await?;
    let db = state.db_handles.writer();
    let Some(row) = repo::by_identifier(db, kind, &value).await? else {
        return Ok(());
    };
    let allowed = match purpose {
        ChallengePurpose::VerifyEmail => {
            AccountStatus::from(row.status) == AccountStatus::PendingVerification
                || (AccountStatus::from(row.status) == AccountStatus::Active
                    && repo::identifiers(db, &AccountId(row.id.clone()))
                        .await?
                        .iter()
                        .any(|i| {
                            IdentifierKind::from(i.kind) == IdentifierKind::Email
                                && i.verified_at.is_none()
                        }))
        }
        ChallengePurpose::ResetPassword => AccountStatus::from(row.status) == AccountStatus::Active,
        ChallengePurpose::ReplaceEmail => false,
    };
    if !allowed {
        return Ok(());
    }
    let id = AccountId(row.id.clone());
    let Some(email) = repo::identifiers(db, &id)
        .await?
        .into_iter()
        .find(|i| IdentifierKind::from(i.kind) == IdentifierKind::Email)
    else {
        return Ok(());
    };
    if purpose == ChallengePurpose::ResetPassword && email.verified_at.is_none() {
        return Ok(());
    }
    let txn = db.begin().await?;
    if !repo::claim_revision(&txn, &AccountId(row.id.clone()), row.revision, now()).await? {
        return Ok(());
    }
    enqueue(state, &txn, &id, purpose, &email.value).await?;
    super::lifecycle::audit(&txn, &id, AuditAction::ChallengeRequested, "anonymous").await?;
    txn.commit().await?;
    Ok(())
}

async fn lookup<C: ConnectionTrait>(
    db: &C,
    token: &str,
    purpose: ChallengePurpose,
) -> Result<challenge::Model> {
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AccountError::InvalidChallenge);
    }
    challenges::live_by_hash(db, &security::digest(token), purpose, now())
        .await?
        .ok_or(AccountError::InvalidChallenge)
}

async fn consume(txn: &sea_orm::DatabaseTransaction, c: &challenge::Model) -> Result<()> {
    if !challenges::consume(txn, &c.id, now()).await? {
        return Err(AccountError::InvalidChallenge);
    }
    Ok(())
}

pub async fn verify_email(state: &AppState, token: &str) -> Result<()> {
    let db = state.db_handles.writer();
    let c = lookup(db, token, ChallengePurpose::VerifyEmail).await?;
    let id = AccountId(c.account_id.clone());
    let row = repo::find(db, &id)
        .await?
        .filter(|r| {
            matches!(
                AccountStatus::from(r.status),
                AccountStatus::PendingVerification | AccountStatus::Active
            )
        })
        .ok_or(AccountError::InvalidChallenge)?;
    let txn = db.begin().await?;
    if !repo::claim_revision(&txn, &AccountId(row.id.clone()), row.revision, now()).await? {
        return Err(AccountError::InvalidChallenge);
    }
    consume(&txn, &c).await?;
    if !writes::verify_email(&txn, &id, &c.target_email, now()).await? {
        return Err(AccountError::InvalidChallenge);
    }
    writes::update_status(&txn, &id, AccountStatus::Active, None).await?;
    cancel(&txn, &id, Some(ChallengePurpose::VerifyEmail)).await?;
    super::lifecycle::audit(&txn, &id, AuditAction::EmailVerified, "mail_challenge").await?;
    txn.commit().await?;
    Ok(())
}

pub async fn reset_password(state: &AppState, token: &str, password: String) -> Result<()> {
    security::validate_password(&password)?;
    let db = state.db_handles.writer();
    let c = lookup(db, token, ChallengePurpose::ResetPassword).await?;
    let id = AccountId(c.account_id.clone());
    security::reserve_attempt(
        state,
        AuthenticationOperation::ResetConsume,
        id.as_str(),
        10,
    )
    .await?;
    let row = repo::find(db, &id)
        .await?
        .filter(|r| AccountStatus::from(r.status) == AccountStatus::Active)
        .ok_or(AccountError::InvalidChallenge)?;
    let hash = state.account_security.hash(password).await?;
    let txn = db.begin().await?;
    if !repo::claim_revision(&txn, &AccountId(row.id.clone()), row.revision, now()).await? {
        return Err(AccountError::InvalidChallenge);
    }
    consume(&txn, &c).await?;
    super::lifecycle::set_password(&txn, &id, hash).await?;
    cancel(&txn, &id, None).await?;
    super::lifecycle::audit(&txn, &id, AuditAction::PasswordReset, "mail_challenge").await?;
    txn.commit().await?;
    Ok(())
}

pub async fn request_email_change(
    state: &AppState,
    principal: &Principal,
    target: &str,
    password: String,
) -> Result<()> {
    ensure_mail(state).await?;
    let target = security::email(target)?;
    let id = AccountId(principal.account_id.0.clone());
    if repo::identifiers(state.db_handles.writer(), &id)
        .await?
        .iter()
        .any(|i| IdentifierKind::from(i.kind) == IdentifierKind::Email && i.value == target)
    {
        return Err(AccountError::InvalidInput);
    }
    security::reserve_attempt(state, AuthenticationOperation::ReplaceEmail, id.as_str(), 3).await?;
    super::lifecycle::prove_password(state, principal, password).await?;
    let txn = security::principal_transaction(state, principal).await?;
    enqueue(state, &txn, &id, ChallengePurpose::ReplaceEmail, &target).await?;
    super::lifecycle::audit(&txn, &id, AuditAction::ChallengeRequested, "self").await?;
    txn.commit().await?;
    Ok(())
}

pub async fn confirm_email_change(
    state: &AppState,
    principal: &Principal,
    token: &str,
) -> Result<()> {
    let c = lookup(
        state.db_handles.writer(),
        token,
        ChallengePurpose::ReplaceEmail,
    )
    .await?;
    if c.account_id != principal.account_id.0 {
        return Err(AccountError::InvalidChallenge);
    }
    let txn = security::principal_transaction(state, principal).await?;
    let id = AccountId(principal.account_id.0.clone());
    consume(&txn, &c).await?;
    writes::replace_email(&txn, &id, &c.target_email, now()).await?;
    cancel(&txn, &id, None).await?;
    repo::revoke_sessions(&txn, &id, now()).await?;
    super::lifecycle::audit(&txn, &id, AuditAction::EmailChanged, "self").await?;
    txn.commit().await?;
    Ok(())
}

pub async fn cleanup(state: &AppState) -> Result<()> {
    challenges::cleanup(state.db_handles.writer(), now()).await?;
    Ok(())
}
