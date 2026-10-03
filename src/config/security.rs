use crate::errors::{AppError, Result};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SecurityConfig {
    #[serde(skip_serializing)]
    pub master_key: String,
}
impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            master_key: hex::encode(rand::random::<[u8; 32]>()),
        }
    }
}
impl std::fmt::Debug for SecurityConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityConfig")
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}
pub struct DeploymentSecrets {
    pub master_key: [u8; 32],
}
pub fn prepare(config: &SecurityConfig) -> Result<DeploymentSecrets> {
    let master_key = decode_master_key(&config.master_key)?;
    Ok(DeploymentSecrets { master_key })
}

pub(crate) fn decode_master_key(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(AppError::Config(
            "master_key must be 64 hexadecimal characters (32 bytes)".into(),
        ));
    }
    let key = hex::decode(value)
        .map_err(|_| AppError::Config("invalid master_key hexadecimal encoding".into()))?;
    key.try_into()
        .map_err(|_| AppError::Config("master_key must decode to 32 bytes".into()))
}
