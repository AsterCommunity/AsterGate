use super::*;

struct FlakyMailSender {
    failures: std::sync::atomic::AtomicUsize,
    captured: aster_forge_mail::MemoryMailSender,
}
#[async_trait::async_trait]
impl aster_forge_mail::MailSender for FlakyMailSender {
    async fn send(
        &self,
        message: aster_forge_mail::MailMessage,
    ) -> aster_forge_mail::MailSendResult<()> {
        if self
            .failures
            .try_update(
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
                |n| n.checked_sub(1),
            )
            .is_ok()
        {
            return Err(aster_forge_mail::MailDeliveryError::Delivery(
                "injected relay failure".into(),
            ));
        }
        aster_forge_mail::MailSender::send(&self.captured, message).await
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test]
async fn email_replacement_requires_ownership_and_revokes_sessions() {
    let f = Fixture::new().await;
    let (_, alice) = f.activate("alice").await;
    let (_, bob) = f.activate("bob").await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/email",
            Some(json!({"email":"new@example.test","password":"wrong"})),
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/email",
            Some(json!({"email":"new@example.test","password":PASSWORD})),
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    let token = f.token().await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/email/confirm",
            Some(json!({"token":token})),
            Some(&bob),
            false
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/email/confirm",
            Some(json!({"token":token})),
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&alice), false)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&bob), false)
            .await
            .0,
        StatusCode::OK
    );
    let new_cookie = f.login("new@example.test", PASSWORD).await;
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&new_cookie), false)
            .await
            .1["data"]["email"],
        "new@example.test"
    );
}

#[tokio::test]
async fn email_conflict_rolls_back_challenge_consumption() {
    let f = Fixture::new().await;
    let (_, alice) = f.activate("alice").await;
    f.activate("bob").await;
    f.request(
        "POST",
        "/api/v1/account/email",
        Some(json!({"email":"bob@example.test","password":PASSWORD})),
        Some(&alice),
        false,
    )
    .await;
    let token = f.token().await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/account/email/confirm",
            Some(json!({"token":token})),
            Some(&alice),
            false
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        f.request("GET", "/api/v1/account", None, Some(&alice), false)
            .await
            .1["data"]["email"],
        "alice@example.test"
    );
    let c = challenge::Entity::find()
        .filter(
            challenge::Column::Purpose
                .eq(aster_gate::db::entities::types::StoredChallengePurpose::ReplaceEmail),
        )
        .one(f.state.db_handles.writer())
        .await
        .unwrap()
        .unwrap();
    assert!(c.consumed_at.is_none());
}

#[tokio::test]
async fn outbox_failure_rolls_back_account_creation() {
    let f = Fixture::new().await;
    f.reject_inserts("mail_outbox").await;
    assert_eq!(
        f.request(
            "POST",
            "/api/v1/auth/register",
            Some(registration("alice")),
            None,
            false
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let db = f.state.db_handles.writer();
    assert_eq!(
        account::Entity::find()
            .filter(account::Column::Id.ne(&f.initial_account_id))
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        identifier::Entity::find()
            .filter(identifier::Column::AccountId.ne(&f.initial_account_id))
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        credential::Entity::find()
            .filter(credential::Column::AccountId.ne(&f.initial_account_id))
            .count(db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(challenge::Entity::find().count(db).await.unwrap(), 0);
}

#[tokio::test]
async fn durable_mail_retries_and_scrubs_tokens_after_delivery() {
    let mut f = Fixture::new().await;
    let sender = Arc::new(FlakyMailSender {
        failures: std::sync::atomic::AtomicUsize::new(1),
        captured: aster_forge_mail::MemoryMailSender::default(),
    });
    f.replace_sender(sender.clone());
    f.register("alice").await;
    aster_gate::services::mail_outbox_service::dispatch_due_with(&f.state)
        .await
        .unwrap();
    let db = f.state.db_handles.writer();
    let row = aster_forge_db::mail_outbox::Entity::find()
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, aster_forge_mail::MailOutboxStatus::Retry);
    assert_eq!(row.attempt_count, 1);
    assert!(sender.captured.messages().is_empty());
    aster_forge_db::mail_outbox::Entity::update_many()
        .col_expr(
            aster_forge_db::mail_outbox::Column::NextAttemptAt,
            Expr::value(chrono::Utc::now()),
        )
        .exec(db)
        .await
        .unwrap();
    aster_gate::services::mail_outbox_service::dispatch_due_with(&f.state)
        .await
        .unwrap();
    let row = aster_forge_db::mail_outbox::Entity::find()
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.status, aster_forge_mail::MailOutboxStatus::Sent);
    assert_eq!(row.payload_json.as_ref(), "{}");
    assert_eq!(sender.captured.messages().len(), 1);
}

#[tokio::test]
async fn expired_mail_is_discarded_and_cleanup_erases_retained_addresses() {
    let f = Fixture::new().await;
    f.register("alice").await;
    let db = f.state.db_handles.writer();
    challenge::Entity::update_many()
        .col_expr(challenge::Column::ExpiresAt, Expr::value(0))
        .exec(db)
        .await
        .unwrap();
    aster_gate::services::mail_outbox_service::dispatch_due_with(&f.state)
        .await
        .unwrap();
    assert!(
        aster_forge_mail::memory_sender_ref(&f.state.mail_sender)
            .unwrap()
            .messages()
            .is_empty()
    );
    aster_gate::services::account::cleanup(&f.state)
        .await
        .unwrap();
    assert_eq!(challenge::Entity::find().count(db).await.unwrap(), 0);
    assert_eq!(
        aster_forge_db::mail_outbox::Entity::find()
            .count(db)
            .await
            .unwrap(),
        0
    );
}
