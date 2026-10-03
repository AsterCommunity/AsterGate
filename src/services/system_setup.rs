use crate::services::authorization::{self, models::SystemPermission};
use crate::{
    db::repository::{
        account_repo, account_writes, system_config_repo as settings_repo,
        system_permission_grant_repo as grants,
    },
    runtime::AppState,
    services::{
        account::{self, AccountError, Result, models::*},
        system_settings::models::*,
    },
};
use sea_orm::TransactionTrait;

pub struct SetupCommand {
    pub settings: SystemSettings,
    pub initial_account: RegisterAccount,
}

pub async fn status(state: &AppState) -> Result<SetupState> {
    let status = settings_repo::setup_state(state.db_handles.writer()).await?;
    if status == SetupState::NeedsSetup
        && !account_repo::list(state.db_handles.writer(), None, 1)
            .await?
            .is_empty()
    {
        return Err(AccountError::Storage);
    }
    Ok(status)
}

pub async fn require_ready(state: &AppState) -> Result<()> {
    if status(state).await? == SetupState::Ready {
        Ok(())
    } else {
        Err(AccountError::SetupRequired)
    }
}

pub async fn initialize(
    state: &AppState,
    command: SetupCommand,
    user_agent: &str,
) -> Result<LoginResult> {
    if status(state).await? == SetupState::Ready {
        return Err(AccountError::AlreadyInitialized);
    }
    let mut settings = command.settings.validate()?;
    settings.revision = 1;
    let username = command.initial_account.username.as_str().to_string();
    let email = command.initial_account.email.as_str().to_string();
    let display_name = command.initial_account.display_name.as_str().to_string();
    let hash = state
        .account_security
        .hash(command.initial_account.password.into_secret())
        .await?;
    let db = state.db_handles.writer();
    let txn = db.begin().await?;
    settings_repo::lock(&txn).await?;
    if settings_repo::setup_state(&txn).await? != SetupState::NeedsSetup {
        return Err(AccountError::AlreadyInitialized);
    }
    if !account_repo::list(&txn, None, 1).await?.is_empty() {
        return Err(AccountError::Storage);
    }
    settings_repo::write(&txn, &settings, &state.account_security.master_key).await?;
    let id = AccountId::new();
    let now = chrono::Utc::now().timestamp();
    account_writes::create_pending(
        &txn,
        &id,
        account_writes::NewAccountRecord {
            username,
            email,
            display_name,
            password_hash: hash,
        },
        now,
    )
    .await?;
    // Initial operator setup never asserts email ownership.
    account_writes::update_status(&txn, &id, AccountStatus::Active, None).await?;
    for permission in SystemPermission::ALL {
        grants::grant(&txn, &id, permission, now).await?;
    }
    settings_repo::complete(&txn).await?;
    let token = account::new_token();
    account_writes::create_session(
        &txn,
        &id,
        account::token_digest(&token),
        user_agent.chars().take(128).collect(),
        now,
        now + settings.session_lifetime.seconds(),
    )
    .await?;
    account::write_audit(&txn, &id, AuditAction::Initialized, "initial_setup").await?;
    let row = account_repo::find(&txn, &id)
        .await?
        .ok_or(AccountError::Storage)?;
    let result = account::project_account(&txn, &row).await?;
    let snapshot = settings_repo::load_snapshot(&txn).await?;
    txn.commit().await?;
    super::system_settings::synchronization::after_commit(state, snapshot).await;
    Ok(LoginResult {
        account: result,
        session_token: token,
    })
}

pub async fn read_settings(state: &AppState, operator: &Principal) -> Result<SystemSettings> {
    authorization::require(
        state.db_handles.writer(),
        operator,
        SystemPermission::SettingsRead,
    )
    .await?;
    state.management_settings()
}

pub(crate) async fn settings_for_update(
    state: &AppState,
    operator: &Principal,
) -> Result<SystemSettings> {
    authorization::require(
        state.db_handles.writer(),
        operator,
        SystemPermission::SettingsUpdate,
    )
    .await?;
    settings_repo::read(
        state.db_handles.writer(),
        &state.account_security.master_key,
    )
    .await
}

pub async fn update_settings(
    state: &AppState,
    operator: &Principal,
    settings: SystemSettings,
    password: Option<String>,
) -> Result<SystemSettings> {
    require_ready(state).await?;
    let mut settings = settings.validate()?;
    let permit =
        authorization::prepare_action(state, operator, SystemPermission::SettingsUpdate, password)
            .await?;
    let txn = state.db_handles.writer().begin().await?;
    settings_repo::lock(&txn).await?;
    permit.recheck(&txn).await?;
    let current = settings_repo::read(&txn, &state.account_security.master_key).await?;
    if current.revision != settings.revision {
        return Err(AccountError::SettingsConflict);
    }
    settings.revision = current
        .revision
        .checked_add(1)
        .ok_or(AccountError::Storage)?;
    settings_repo::write(&txn, &settings, &state.account_security.master_key).await?;
    let id = operator.account_id.clone();
    account::write_audit(&txn, &id, AuditAction::SettingsUpdated, "instance_operator").await?;
    let snapshot = settings_repo::load_snapshot(&txn).await?;
    txn.commit().await?;
    super::system_settings::synchronization::after_commit(state, snapshot).await;
    Ok(settings)
}
