pub mod models;
#[cfg(test)]
mod tests;

use crate::{
    db::repository::{account_repo, system_permission_grant_repo as grants},
    runtime::AppState,
    services::account::{
        AccountError, Result,
        models::{AccountId, AccountStatus, Principal},
    },
};
use models::{AuthenticationRequirement, SystemPermission};

/// Exact allow-list membership; permission prefixes, wildcards, and other scopes never grant access.
pub async fn require<C: sea_orm::ConnectionTrait>(
    db: &C,
    principal: &Principal,
    permission: SystemPermission,
) -> Result<()> {
    let account = account_repo::find(db, &principal.account_id)
        .await?
        .ok_or(AccountError::Unauthenticated)?;
    let session =
        account_repo::session_by_token(db, &principal.token_hash, chrono::Utc::now().timestamp())
            .await?
            .ok_or(AccountError::Unauthenticated)?;
    if AccountStatus::from(account.status) != AccountStatus::Active
        || session.account_id != principal.account_id.as_str()
    {
        return Err(AccountError::Unauthenticated);
    }
    if !grants::list(db, &principal.account_id)
        .await?
        .contains(&permission)
    {
        return Err(AccountError::Forbidden);
    }
    Ok(())
}

pub(crate) async fn prepare_action<'a>(
    state: &AppState,
    principal: &'a Principal,
    permission: SystemPermission,
    password: Option<String>,
) -> Result<SystemActionPermit<'a>> {
    require(state.db_handles.writer(), principal, permission).await?;
    if permission.definition().authentication == AuthenticationRequirement::CurrentPassword {
        crate::services::account::verify_current_password(
            state,
            principal,
            password.ok_or(AccountError::InvalidCredentials)?,
        )
        .await?;
    }
    Ok(SystemActionPermit {
        principal,
        permission,
    })
}

pub(crate) struct SystemActionPermit<'a> {
    principal: &'a Principal,
    permission: SystemPermission,
}
impl SystemActionPermit<'_> {
    /// Revalidate after locking the system mutation row; a previous password check is not a grant.
    pub(crate) async fn recheck(self, db: &sea_orm::DatabaseTransaction) -> Result<()> {
        require(db, self.principal, self.permission).await?;
        if !account_repo::claim_revision(
            db,
            &self.principal.account_id,
            self.principal.revision,
            chrono::Utc::now().timestamp(),
        )
        .await?
        {
            return Err(AccountError::Conflict);
        }
        Ok(())
    }
}

/// Called under the shared system mutation lock, before disabling/deleting an active Account.
pub(crate) async fn protect_last_operator<C: sea_orm::ConnectionTrait>(
    db: &C,
    id: &AccountId,
) -> Result<()> {
    let account = account_repo::find(db, id)
        .await?
        .ok_or(AccountError::NotFound)?;
    if AccountStatus::from(account.status) != AccountStatus::Active {
        return Ok(());
    }
    for permission in grants::list(db, id).await? {
        if permission.definition().protects_last_holder
            && grants::active_holders(db, permission).await? <= 1
        {
            return Err(AccountError::LastOperator);
        }
    }
    Ok(())
}
