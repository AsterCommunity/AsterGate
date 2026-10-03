#[test]
fn account_domain_and_commands_have_no_wire_or_persistence_dependencies() {
    let models = include_str!("../../src/services/account/models.rs");
    for forbidden in [
        "axum",
        "serde",
        "sea_orm",
        "db::entities",
        "Json",
        "HeaderMap",
    ] {
        assert!(
            !models.contains(forbidden),
            "Account domain depends on {forbidden}"
        );
    }
}

#[test]
fn system_authorization_has_no_transport_or_orm_dependencies_in_its_models() {
    let models = include_str!("../../src/services/authorization/models.rs");
    for forbidden in ["axum", "serde", "sea_orm", "db::entities", "HeaderMap"] {
        assert!(!models.contains(forbidden));
    }
    let transport = include_str!("../../src/api/account_security.rs");
    assert!(
        !transport.contains("management/settings"),
        "Transport must not infer permissions from the URL"
    );
}

#[test]
fn account_services_delegate_queries_and_conditional_writes_to_repositories() {
    for source in [
        include_str!("../../src/services/account/lifecycle.rs"),
        include_str!("../../src/services/account/challenges.rs"),
        include_str!("../../src/services/account/security.rs"),
    ] {
        for forbidden in [
            "::Entity::",
            "::ActiveModel",
            ".col_expr(",
            "QueryFilter",
            "sea_query::",
        ] {
            assert!(
                !source.contains(forbidden),
                "Account service owns persistence operation {forbidden}"
            );
        }
    }
}

#[test]
fn account_model_is_global_and_contains_no_drive_domain_entities() {
    let source = include_str!("../../src/db/entities/account.rs");
    for forbidden in [
        "organization_id",
        "workspace",
        "storage_quota",
        "policy_group",
        "is_admin",
    ] {
        assert!(
            !source.contains(forbidden),
            "Global Account includes {forbidden}"
        );
    }
}
