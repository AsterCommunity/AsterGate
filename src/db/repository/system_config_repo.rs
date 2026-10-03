use crate::{
    config::{
        definitions::*,
        system_config::{CONFIG_REGISTRY, CONFIG_STORE},
    },
    services::{
        account::{AccountError, Result},
        system_settings::{branding::*, mail::*, models::*, policy::*, site::PublicSiteUrls},
    },
};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, sea_query::Expr};

const SMTP_SECRET_CONTEXT: &[u8] = b"astergate.smtp-password.v1";

pub async fn seed<C: ConnectionTrait>(db: &C) -> Result<()> {
    CONFIG_STORE
        .ensure_defaults(db)
        .await
        .map_err(|_| AccountError::Storage)?;
    Ok(())
}

async fn value<C: ConnectionTrait>(db: &C, key: &str) -> Result<String> {
    CONFIG_STORE
        .find_by_key(db, key)
        .await
        .map_err(|_| AccountError::Storage)?
        .map(|v| v.value)
        .ok_or(AccountError::Storage)
}

pub async fn setup_state<C: ConnectionTrait>(db: &C) -> Result<SetupState> {
    match value(db, SETUP_COMPLETED_KEY).await?.as_str() {
        "false" => Ok(SetupState::NeedsSetup),
        "true" => Ok(SetupState::Ready),
        _ => Err(AccountError::Storage),
    }
}

pub async fn lock<C: ConnectionTrait>(db: &C) -> Result<()> {
    use aster_forge_db::system_config::{Column, Entity};
    // An actual update takes SQLite's write lock too; SELECT FOR UPDATE is insufficient there.
    Entity::update_many()
        .col_expr(Column::Value, Expr::col(Column::Value))
        .filter(Column::Key.eq(SETUP_COMPLETED_KEY))
        .exec(db)
        .await?;
    Ok(())
}

pub type ConfigSnapshot =
    aster_forge_config::SyncConfigSnapshot<aster_forge_db::system_config::Model>;

pub async fn load_snapshot<C: ConnectionTrait>(db: &C) -> Result<ConfigSnapshot> {
    use aster_forge_db::system_config::{Column, Entity};
    let rows = Entity::find()
        .filter(
            Column::Key.is_in(
                CONFIG_REGISTRY
                    .definitions()
                    .iter()
                    .map(|definition| definition.key),
            ),
        )
        .all(db)
        .await?;
    Ok(ConfigSnapshot::from_configs(rows))
}
fn get<'a>(snapshot: &'a ConfigSnapshot, key: &str) -> Result<&'a str> {
    snapshot.get(key).ok_or(AccountError::Storage)
}
fn number(snapshot: &ConfigSnapshot, key: &str) -> Result<i64> {
    get(snapshot, key)?
        .parse()
        .map_err(|_| AccountError::Storage)
}
pub fn snapshot_revision(snapshot: &ConfigSnapshot) -> Result<u64> {
    get(snapshot, SETTINGS_REVISION_KEY)?
        .parse()
        .map_err(|_| AccountError::Storage)
}
pub fn decode_branding(snapshot: &ConfigSnapshot) -> Result<PublicBranding> {
    Ok(PublicBranding {
        title: BrandingTitle::parse(get(snapshot, BRANDING_TITLE_KEY)?)
            .map_err(|_| AccountError::Storage)?,
        description: BrandingDescription::parse(get(snapshot, BRANDING_DESCRIPTION_KEY)?)
            .map_err(|_| AccountError::Storage)?,
    })
}
pub fn decode_authentication(snapshot: &ConfigSnapshot) -> Result<AuthenticationSettings> {
    match get(snapshot, SETUP_COMPLETED_KEY)? {
        "false" => return Err(AccountError::SetupRequired),
        "true" => {}
        _ => return Err(AccountError::Storage),
    }
    let settings = AuthenticationSettings {
        revision: snapshot_revision(snapshot)?,
        site_urls: PublicSiteUrls::parse(
            &serde_json::from_str::<Vec<String>>(get(snapshot, PUBLIC_SITE_URLS_KEY)?)
                .map_err(|_| AccountError::Storage)?,
        )?,
        registration: RegistrationPolicy::parse(get(snapshot, ACCOUNT_REGISTRATION_KEY)?)?,
        email_verification: EmailVerificationPolicy::parse(get(
            snapshot,
            ACCOUNT_EMAIL_VERIFICATION_KEY,
        )?)?,
        session_lifetime: SessionLifetime::new(number(snapshot, ACCOUNT_SESSION_TTL_KEY)?)?,
        challenge_lifetime: ChallengeLifetime::new(number(snapshot, ACCOUNT_CHALLENGE_TTL_KEY)?)?,
        mail_enabled: !get(snapshot, MAIL_SMTP_HOST_KEY)?.trim().is_empty(),
    };
    validate_registration_delivery(
        settings.registration,
        settings.email_verification,
        settings.mail_enabled,
    )?;
    Ok(settings)
}
pub fn decode_mail(snapshot: &ConfigSnapshot, master_key: &[u8]) -> Result<MailSettings> {
    let encrypted = get(snapshot, MAIL_SMTP_PASSWORD_KEY)?;
    let password = if encrypted.is_empty() {
        SecretString::default()
    } else {
        SecretString::new(
            String::from_utf8(aster_forge_crypto::decrypt_secret(
                master_key,
                SMTP_SECRET_CONTEXT,
                MAIL_SMTP_PASSWORD_KEY.as_bytes(),
                encrypted,
            )?)
            .map_err(|_| AccountError::Security)?,
        )?
    };
    MailSettings {
        host: get(snapshot, MAIL_SMTP_HOST_KEY)?.into(),
        port: u16::try_from(number(snapshot, MAIL_SMTP_PORT_KEY)?)
            .map_err(|_| AccountError::Storage)?,
        username: get(snapshot, MAIL_SMTP_USERNAME_KEY)?.into(),
        password,
        from_address: get(snapshot, MAIL_FROM_ADDRESS_KEY)?.into(),
        from_name: get(snapshot, MAIL_FROM_NAME_KEY)?.into(),
        security: SmtpSecurity::parse(get(snapshot, MAIL_SMTP_SECURITY_KEY)?)?,
    }
    .validate()
    .map_err(|_| AccountError::Storage)
}
pub fn decode_settings(snapshot: &ConfigSnapshot, master_key: &[u8]) -> Result<SystemSettings> {
    let authentication = decode_authentication(snapshot)?;
    SystemSettings {
        revision: authentication.revision,
        site_urls: authentication.site_urls,
        branding: decode_branding(snapshot)?,
        registration: authentication.registration,
        email_verification: authentication.email_verification,
        session_lifetime: authentication.session_lifetime,
        challenge_lifetime: authentication.challenge_lifetime,
        mail: decode_mail(snapshot, master_key)?,
    }
    .validate()
    .map_err(|_| AccountError::Storage)
}
pub async fn read<C: ConnectionTrait>(db: &C, master_key: &[u8]) -> Result<SystemSettings> {
    decode_settings(&load_snapshot(db).await?, master_key)
}
pub async fn read_authentication<C: ConnectionTrait>(db: &C) -> Result<AuthenticationSettings> {
    decode_authentication(&load_snapshot(db).await?)
}

pub async fn write<C: ConnectionTrait>(
    db: &C,
    settings: &SystemSettings,
    master_key: &[u8],
) -> Result<()> {
    let password = if settings.mail.password.is_empty() {
        String::new()
    } else {
        aster_forge_crypto::encrypt_secret(
            master_key,
            SMTP_SECRET_CONTEXT,
            MAIL_SMTP_PASSWORD_KEY.as_bytes(),
            settings.mail.password.expose().as_bytes(),
        )?
    };
    let values = [
        (SETTINGS_REVISION_KEY, settings.revision.to_string()),
        (BRANDING_TITLE_KEY, settings.branding.title.as_str().into()),
        (
            BRANDING_DESCRIPTION_KEY,
            settings.branding.description.as_str().into(),
        ),
        (
            PUBLIC_SITE_URLS_KEY,
            serde_json::to_string(&settings.site_urls.values())
                .map_err(|_| AccountError::Storage)?,
        ),
        (
            ACCOUNT_EMAIL_VERIFICATION_KEY,
            settings.email_verification.as_str().into(),
        ),
        (
            ACCOUNT_REGISTRATION_KEY,
            settings.registration.as_str().into(),
        ),
        (
            ACCOUNT_SESSION_TTL_KEY,
            settings.session_lifetime.seconds().to_string(),
        ),
        (
            ACCOUNT_CHALLENGE_TTL_KEY,
            settings.challenge_lifetime.seconds().to_string(),
        ),
        (MAIL_SMTP_HOST_KEY, settings.mail.host.clone()),
        (MAIL_SMTP_PORT_KEY, settings.mail.port.to_string()),
        (MAIL_SMTP_USERNAME_KEY, settings.mail.username.clone()),
        (MAIL_SMTP_PASSWORD_KEY, password),
        (MAIL_FROM_ADDRESS_KEY, settings.mail.from_address.clone()),
        (MAIL_FROM_NAME_KEY, settings.mail.from_name.clone()),
        (
            MAIL_SMTP_SECURITY_KEY,
            settings.mail.security.as_str().into(),
        ),
    ];
    let lookup: std::collections::HashMap<String, String> = values
        .iter()
        .map(|(key, value)| ((*key).into(), value.clone()))
        .collect();
    let normalized = values
        .iter()
        .map(|(key, value)| {
            CONFIG_REGISTRY
                .normalize_value(&lookup, key, value)
                .map(|value| (*key, value))
                .map_err(|_| AccountError::InvalidInput)
        })
        .collect::<Result<Vec<_>>>()?;
    for (key, value) in normalized {
        set(db, key, &value).await?;
    }
    Ok(())
}

pub async fn complete<C: ConnectionTrait>(db: &C) -> Result<()> {
    set(db, SETUP_COMPLETED_KEY, "true").await
}

async fn set<C: ConnectionTrait>(db: &C, key: &str, value: &str) -> Result<()> {
    CONFIG_STORE
        .upsert(
            db,
            aster_forge_db::system_config::SystemConfigUpsert {
                key,
                value,
                visibility: None,
                updated_by: None,
            },
        )
        .await
        .map_err(|_| AccountError::Storage)?;
    Ok(())
}
