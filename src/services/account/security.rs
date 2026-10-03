use super::{AccountError, Result, models::*, now};
use crate::runtime::AppState;

use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct AccountSecurity {
    hashing_slots: Arc<Semaphore>,
    dummy_hash: String,
    pub(crate) master_key: [u8; 32],
}

impl AccountSecurity {
    pub async fn new(config: &crate::config::AppConfig) -> crate::errors::Result<Self> {
        let secrets = crate::config::security::prepare(&config.security)?;
        let dummy_hash =
            tokio::task::spawn_blocking(|| aster_forge_crypto::hash_password(&random_token()))
                .await
                .map_err(|_| crate::errors::AppError::Runtime("password worker failed".into()))?
                .map_err(|_| crate::errors::AppError::Runtime("password hashing failed".into()))?;
        Ok(Self {
            hashing_slots: Arc::new(Semaphore::new(4)),
            dummy_hash,
            master_key: secrets.master_key,
        })
    }

    pub(crate) async fn hash(&self, password: String) -> Result<String> {
        let permit = self
            .hashing_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| AccountError::RateLimited)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            aster_forge_crypto::hash_password(&password)
        })
        .await
        .map_err(|_| AccountError::Security)?
        .map_err(Into::into)
    }

    pub(crate) async fn verify(&self, password: String, hash: Option<String>) -> Result<bool> {
        let hash = hash.unwrap_or_else(|| self.dummy_hash.clone());
        let permit = self
            .hashing_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| AccountError::RateLimited)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            aster_forge_crypto::verify_password(&password, &hash)
        })
        .await
        .map_err(|_| AccountError::Security)?
        .map_err(Into::into)
    }
}

pub(crate) fn random_token() -> String {
    aster_forge_crypto::bytes_to_hex(&rand::random::<[u8; 32]>())
}

pub(crate) fn digest(value: &str) -> String {
    aster_forge_crypto::sha256_hex(value.as_bytes())
}

pub(crate) fn validate_password(value: &str) -> Result<()> {
    if (12..=128).contains(&value.len()) {
        Ok(())
    } else {
        Err(AccountError::InvalidInput)
    }
}

pub(crate) fn username(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    if !(3..=32).contains(&value.len())
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(AccountError::InvalidInput);
    }
    Ok(value)
}

pub(crate) fn email(value: &str) -> Result<String> {
    aster_forge_validation::email::normalize_email(value).map_err(|_| AccountError::InvalidInput)
}

pub(crate) fn display_name(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(AccountError::InvalidInput);
    }
    Ok(value.into())
}

pub(crate) fn login_identifier(value: &str) -> Result<(IdentifierKind, String)> {
    if value.contains('@') {
        Ok((IdentifierKind::Email, email(value)?))
    } else {
        Ok((IdentifierKind::Username, username(value)?))
    }
}

/// Reservations are committed before expensive work and survive failed authentication/restarts.
pub(crate) async fn reserve_attempt(
    state: &AppState,
    operation: AuthenticationOperation,
    identifier: &str,
    limit: i32,
) -> Result<()> {
    let clock = now();
    let material = format!("{}:{identifier}", operation.as_str());
    let hash = aster_forge_crypto::hmac_sha256_hex(
        &state.account_security.master_key,
        material.as_bytes(),
    )?;
    let key = format!("{}:{hash}", clock / 900);
    let db = state.db_handles.writer();
    if crate::db::repository::authentication_budget_repo::reserve(
        db,
        key,
        (clock / 900 + 1) * 900,
        limit,
    )
    .await?
    {
        Ok(())
    } else {
        Err(AccountError::RateLimited)
    }
}

pub(crate) async fn principal_transaction(
    state: &AppState,
    principal: &Principal,
) -> Result<sea_orm::DatabaseTransaction> {
    use sea_orm::TransactionTrait;
    let txn = state.db_handles.writer().begin().await?;
    let session =
        crate::db::repository::account_repo::session_by_token(&txn, &principal.token_hash, now())
            .await?;
    if session.is_none() {
        return Err(AccountError::Unauthenticated);
    }
    if !crate::db::repository::account_repo::claim_revision(
        &txn,
        &principal.account_id,
        principal.revision,
        now(),
    )
    .await?
    {
        return Err(AccountError::Conflict);
    }
    Ok(txn)
}
