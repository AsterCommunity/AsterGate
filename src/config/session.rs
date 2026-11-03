//! Stable first-party signing namespace, independent of editable public sites and AEAD keys.
use crate::errors::{AppError, Result};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionConfig {
    pub issuer: String,
    #[serde(skip_serializing)]
    pub signing_key: String,
}

impl Default for SessionConfig {
    /// Generates an independent random signing key and a loopback development issuer.
    /// The configuration loader persists the key; production deployments must set a stable issuer.
    fn default() -> Self {
        Self {
            issuer: "http://localhost:3000".into(),
            signing_key: hex::encode(rand::random::<[u8; 32]>()),
        }
    }
}

impl std::fmt::Debug for SessionConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionConfig")
            .field("issuer", &self.issuer)
            .field("signing_key", &"[REDACTED]")
            .finish()
    }
}

impl SessionConfig {
    /// Validates an explicit HTTPS or loopback issuer and decodes exactly 32 bytes of signing material.
    /// Malformed configuration fails closed without echoing the key in errors.
    pub(crate) fn key(&self) -> Result<aster_forge_auth::JwtKey> {
        // Issuer is explicit deployment identity, never inferred from request/site headers.
        let url = url::Url::parse(&self.issuer).map_err(|_| {
            AppError::Config("session.issuer must be an absolute HTTP(S) URL".into())
        })?;
        if url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.host_str().is_none()
            || !(url.scheme() == "https"
                || (url.scheme() == "http"
                    && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))))
        {
            return Err(AppError::Config("session.issuer requires HTTPS or HTTP loopback, without credentials/query/fragment".into()));
        }
        let key = super::security::decode_master_key(&self.signing_key).map_err(|_| {
            AppError::Config(
                "session.signing_key must encode 32 random bytes as 64 hex characters".into(),
            )
        })?;
        aster_forge_auth::JwtKey::hmac(key)
            .map_err(|_| AppError::Config("invalid session signing key".into()))
    }
}
