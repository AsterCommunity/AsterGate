//! Product mail outbox boundary.
//!
//! Forge owns the shared outbox state machine. Product code should keep template payloads,
//! rendering, business URLs, and audit hooks here.

use crate::errors::Result;

const MAIL_OUTBOX_DISPATCH_CONFIG: aster_forge_mail::MailOutboxDispatchConfig =
    aster_forge_mail::MailOutboxDispatchConfig::new(
        20,
        60,
        32,
        aster_forge_mail::MailOutboxRetryPolicy::new(6, aster_forge_mail::DEFAULT_ERROR_MAX_LEN),
    );

/// Dispatches due mail outbox rows using the Forge DB-backed state machine.
pub async fn dispatch_due_with(
    state: &crate::runtime::AppState,
) -> Result<aster_forge_mail::DispatchStats> {
    let store = aster_forge_db::MailOutboxDbStore::new(state.db_handles.writer().clone());
    store
        .dispatch_due(
            &MAIL_OUTBOX_DISPATCH_CONFIG,
            |row| async move { deliver_one(state, &row).await },
            |context, attempt_count, subject| async move {
                tracing::info!(
                    mail_outbox_id = context.id,
                    attempt_count,
                    subject,
                    "mail sent"
                );
            },
            |context, attempt_count, _error_message| async move {
                tracing::warn!(
                    mail_outbox_id = context.id,
                    attempt_count,
                    "mail delivery permanently failed"
                );
            },
        )
        .await
}

async fn deliver_one(
    state: &crate::runtime::AppState,
    row: &aster_forge_db::mail_outbox::Model,
) -> Result<String> {
    use crate::services::account::challenges::{MAIL_CONTEXT, MailPayload};
    use sea_orm::EntityTrait;
    let payload: MailPayload = serde_json::from_str(row.payload_json.as_ref())
        .map_err(|_| crate::errors::AppError::Mail("invalid account mail payload".into()))?;
    let challenge =
        crate::db::entities::account_challenge::Entity::find_by_id(&payload.challenge_id)
            .one(state.db_handles.writer())
            .await?;
    let Some(challenge) = challenge
        .filter(|c| c.consumed_at.is_none() && c.expires_at > chrono::Utc::now().timestamp())
    else {
        return Ok("expired account message discarded".into());
    };
    let key = &state.account_security.master_key;
    let token = aster_forge_crypto::decrypt_secret(
        key,
        MAIL_CONTEXT,
        payload.challenge_id.as_bytes(),
        &payload.envelope,
    )
    .map_err(|_| crate::errors::AppError::Mail("mail payload authentication failed".into()))?;
    let token = String::from_utf8(token)
        .map_err(|_| crate::errors::AppError::Mail("invalid mail token".into()))?;
    let (path, subject) =
        match crate::services::account::models::ChallengePurpose::from(challenge.purpose) {
            crate::services::account::models::ChallengePurpose::VerifyEmail => {
                ("verify-email", "Verify your AsterGate email")
            }
            crate::services::account::models::ChallengePurpose::ResetPassword => {
                ("reset-password", "Reset your AsterGate password")
            }
            crate::services::account::models::ChallengePurpose::ReplaceEmail => {
                ("confirm-email", "Confirm your AsterGate email change")
            }
        };
    let authentication = state
        .runtime_config
        .authentication()
        .map_err(|_| crate::errors::AppError::Mail("mail settings unavailable".into()))?;
    let mail = state
        .runtime_config
        .mail()
        .map_err(|_| crate::errors::AppError::Mail("mail settings unavailable".into()))?;
    let link = authentication.site_urls.primary().link(path, &token);
    let html_link = aster_forge_utils::html::escape_html(&link);
    let subject = subject.to_string();
    state.mail_sender
        .send(aster_forge_mail::MailMessage {
            from: aster_forge_mail::MailRecipient {
                address: mail.from_address.clone(),
                display_name: Some(mail.from_name.clone()),
            },
            to: aster_forge_mail::MailRecipient {
                address: row.to_address.clone(),
                display_name: row.to_name.clone(),
            },
            subject: subject.clone(),
            text_body: format!("{subject}\n\n{link}\n\nThis link expires at {} UTC or when replaced. If you did not request it, ignore this message.", chrono::DateTime::from_timestamp(challenge.expires_at, 0).map(|t| t.to_rfc3339()).unwrap_or_default()),
            html_body: format!("<p>{subject}</p><p><a href=\"{html_link}\">Continue</a></p><p>If you did not request this, ignore this message.</p>"),
        })
        .await.map_err(|_| crate::errors::AppError::Mail("mail delivery failed".into()))?;
    Ok(subject)
}

pub mod runtime {
    //! Mail outbox runtime component integration.

    /// Creates the mail outbox runtime component used by the product entrypoint.
    pub fn mail_runtime_component(
        state: std::sync::Arc<crate::runtime::AppState>,
    ) -> aster_forge_runtime::RuntimeComponentBundleRegistration<
        aster_forge_runtime::ShutdownResourceComponent<MailOutboxRuntimeResources>,
    > {
        aster_forge_mail::mail_outbox_component(
            MailOutboxRuntimeResources { state },
            |resources| async move {
                super::dispatch_due_with(&resources.state)
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
        )
    }

    /// Minimal resources needed for shutdown drain.
    pub struct MailOutboxRuntimeResources {
        state: std::sync::Arc<crate::runtime::AppState>,
    }
}
