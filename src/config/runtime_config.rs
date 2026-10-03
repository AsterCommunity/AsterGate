//! Atomic Forge snapshots with independently validated, non-secret request projections.
use crate::{
    db::repository::system_config_repo::{self as repository, ConfigSnapshot},
    services::{
        account::{AccountError, Result},
        system_settings::{
            branding::PublicBranding,
            mail::MailSettings,
            models::{AuthenticationSettings, SystemSettings},
        },
    },
};
use std::sync::{Arc, OnceLock, RwLock};

struct Snapshot {
    source: ConfigSnapshot,
    revision: Option<u64>,
    authentication: Result<AuthenticationSettings>,
    branding: Result<PublicBranding>,
    mail: OnceLock<Result<MailSettings>>,
}
impl Snapshot {
    fn new(source: ConfigSnapshot) -> Self {
        Self {
            revision: repository::snapshot_revision(&source).ok(),
            authentication: repository::decode_authentication(&source).map_err(
                |error| match error {
                    AccountError::SetupRequired => error,
                    _ => AccountError::Storage,
                },
            ),
            branding: repository::decode_branding(&source),
            source,
            mail: OnceLock::new(),
        }
    }
}

pub struct RuntimeConfig {
    master_key: [u8; 32],
    snapshot: RwLock<Arc<Snapshot>>,
    reload_lock: tokio::sync::Mutex<()>,
}
impl RuntimeConfig {
    pub fn new(source: ConfigSnapshot, master_key: [u8; 32]) -> Self {
        Self {
            master_key,
            snapshot: RwLock::new(Arc::new(Snapshot::new(source))),
            reload_lock: tokio::sync::Mutex::new(()),
        }
    }
    fn snapshot(&self) -> Result<Arc<Snapshot>> {
        self.snapshot
            .read()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| AccountError::Storage)
    }
    pub fn apply_snapshot(&self, source: ConfigSnapshot) -> Result<()> {
        let next = Snapshot::new(source);
        let mut current = self.snapshot.write().map_err(|_| AccountError::Storage)?;
        // Late post-commit refreshes must never roll a replica back to an older revision.
        if matches!((current.revision, next.revision), (Some(current), Some(next)) if next < current)
            || current.source == next.source
        {
            return Ok(());
        }
        *current = Arc::new(next);
        Ok(())
    }
    pub async fn reload<C: sea_orm::ConnectionTrait>(&self, db: &C) -> Result<()> {
        let _guard = self.reload_lock.lock().await;
        self.apply_snapshot(repository::load_snapshot(db).await?)
    }
    pub fn authentication(&self) -> Result<AuthenticationSettings> {
        self.snapshot()?.authentication.clone()
    }
    pub fn branding(&self) -> Result<PublicBranding> {
        self.snapshot()?.branding.clone()
    }
    pub fn mail(&self) -> Result<MailSettings> {
        let snapshot = self.snapshot()?;
        snapshot
            .mail
            .get_or_init(|| repository::decode_mail(&snapshot.source, &self.master_key))
            .clone()
    }
    pub fn management_settings(&self) -> Result<SystemSettings> {
        let snapshot = self.snapshot()?;
        let authentication = snapshot.authentication.clone()?;
        SystemSettings {
            revision: authentication.revision,
            site_urls: authentication.site_urls,
            branding: snapshot.branding.clone()?,
            registration: authentication.registration,
            email_verification: authentication.email_verification,
            session_lifetime: authentication.session_lifetime,
            challenge_lifetime: authentication.challenge_lifetime,
            mail: snapshot
                .mail
                .get_or_init(|| repository::decode_mail(&snapshot.source, &self.master_key))
                .clone()?,
        }
        .validate()
    }
}
