use sea_orm::entity::prelude::*;

/// Persistence boundary: ORM-generated ActiveModel Debug must not disclose refresh credentials.
#[derive(Clone, PartialEq, DeriveValueType)]
pub struct StoredRefreshJti(String);

impl From<&aster_forge_auth::RefreshJti> for StoredRefreshJti {
    fn from(jti: &aster_forge_auth::RefreshJti) -> Self {
        Self(jti.as_str().to_owned())
    }
}

impl StoredRefreshJti {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for StoredRefreshJti {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Clone, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "browser_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub account_id: String,
    pub session_version: i64,
    pub current_refresh_jti: StoredRefreshJti,
    pub previous_refresh_jti: Option<StoredRefreshJti>,
    pub last_rotated_at: i64,
    pub user_agent: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSession")
            .field("id", &self.id)
            .field("account_id", &self.account_id)
            .field("refresh", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
