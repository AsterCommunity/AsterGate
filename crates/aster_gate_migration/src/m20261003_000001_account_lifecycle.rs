use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn name(value: &str) -> Alias {
    Alias::new(value)
}
fn id(value: &str) -> ColumnDef {
    let mut c = ColumnDef::new(name(value));
    c.string_len(36).not_null();
    c
}
fn text(value: &str, len: u32) -> ColumnDef {
    let mut c = ColumnDef::new(name(value));
    c.string_len(len).not_null();
    c
}
fn time(value: &str) -> ColumnDef {
    let mut c = ColumnDef::new(name(value));
    c.big_integer().not_null();
    c
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(name("accounts"))
                    .col(id("id").primary_key())
                    .col(text("display_name", 128))
                    .col(text("status", 24))
                    .col(time("revision"))
                    .col(time("created_at"))
                    .col(time("updated_at"))
                    .col(ColumnDef::new(name("suspended_from")).string_len(24))
                    .to_owned(),
            )
            .await?;

        let mut identifiers = Table::create();
        identifiers
            .table(name("account_identifiers"))
            .col(id("id").primary_key())
            .col(id("account_id"))
            .col(text("kind", 16))
            .col(text("value", 254))
            .col(ColumnDef::new(name("verified_at")).big_integer());
        add_account_fk(&mut identifiers, "account_identifiers");
        identifiers
            .index(
                Index::create()
                    .name("uq_identifier_value")
                    .unique()
                    .col(name("kind"))
                    .col(name("value")),
            )
            .index(
                Index::create()
                    .name("uq_account_identifier_kind")
                    .unique()
                    .col(name("account_id"))
                    .col(name("kind")),
            );
        manager.create_table(identifiers.to_owned()).await?;

        let mut credentials = Table::create();
        credentials
            .table(name("password_credentials"))
            .col(id("account_id").primary_key())
            .col(text("password_hash", 512))
            .col(time("updated_at"));
        add_account_fk(&mut credentials, "password_credentials");
        manager.create_table(credentials.to_owned()).await?;
        let mut grants = Table::create();
        grants
            .table(name("system_permission_grants"))
            .col(id("account_id"))
            .col(text("permission_key", 128))
            .col(time("created_at"))
            .primary_key(
                Index::create()
                    .col(name("account_id"))
                    .col(name("permission_key")),
            );
        add_account_fk(&mut grants, "system_permission_grants");
        manager.create_table(grants.to_owned()).await?;

        let mut sessions = Table::create();
        sessions
            .table(name("browser_sessions"))
            .col(id("id").primary_key())
            .col(id("account_id"))
            .col(text("token_hash", 64).unique_key())
            .col(text("user_agent", 512))
            .col(time("created_at"))
            .col(time("expires_at"))
            .col(ColumnDef::new(name("revoked_at")).big_integer());
        add_account_fk(&mut sessions, "browser_sessions");
        manager.create_table(sessions.to_owned()).await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_account_sessions")
                    .table(name("browser_sessions"))
                    .col(name("account_id"))
                    .col(name("expires_at"))
                    .to_owned(),
            )
            .await?;

        let mut challenges = Table::create();
        challenges
            .table(name("account_challenges"))
            .col(id("id").primary_key())
            .col(id("account_id"))
            .col(text("purpose", 24))
            .col(text("token_hash", 64).unique_key())
            .col(text("target_email", 254))
            .col(time("created_at"))
            .col(time("expires_at"))
            .col(ColumnDef::new(name("consumed_at")).big_integer())
            .col(ColumnDef::new(name("outbox_id")).big_integer());
        add_account_fk(&mut challenges, "account_challenges");
        manager.create_table(challenges.to_owned()).await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_account_challenges")
                    .table(name("account_challenges"))
                    .col(name("account_id"))
                    .col(name("purpose"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(name("authentication_budgets"))
                    .col(text("key", 80).primary_key())
                    .col(ColumnDef::new(name("attempts")).integer().not_null())
                    .col(time("expires_at"))
                    .to_owned(),
            )
            .await?;
        for (table, index) in [
            ("browser_sessions", "idx_sessions_expiry"),
            ("account_challenges", "idx_challenges_expiry"),
            ("authentication_budgets", "idx_budgets_expiry"),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(index)
                        .table(name(table))
                        .col(name("expires_at"))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "authentication_budgets",
            "account_challenges",
            "browser_sessions",
            "password_credentials",
            "system_permission_grants",
            "account_identifiers",
            "accounts",
        ] {
            manager
                .drop_table(Table::drop().table(name(table)).to_owned())
                .await?;
        }
        Ok(())
    }
}

fn add_account_fk(table: &mut TableCreateStatement, table_name: &str) {
    table.foreign_key(
        ForeignKey::create()
            .name(format!("fk_{table_name}_account"))
            .from(name(table_name), name("account_id"))
            .to(name("accounts"), name("id")),
    );
}
