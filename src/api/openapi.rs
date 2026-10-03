//! OpenAPI document assembly.
//!
//! Product routes should add their annotated handlers and DTO schemas here. The module is compiled
//! only for debug builds with the `openapi` feature, keeping normal release binaries free of API
//! documentation generation overhead.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
        description = env!("CARGO_PKG_DESCRIPTION"),
        license(name = "MIT"),
    ),
    modifiers(&AccountApiContract),
    paths(
        crate::api::routes::health::health,
        crate::api::routes::health::ready,
        crate::api::routes::public::frontend_config,
        crate::api::routes::system::status,
        crate::api::routes::system::initialize,
        crate::api::routes::system::settings,
        crate::api::routes::system::update_settings,
        crate::api::routes::account::register,
        crate::api::routes::account::login,
        crate::api::routes::account::logout,
        crate::api::routes::account::request_verification,
        crate::api::routes::account::verify_email,
        crate::api::routes::account::request_recovery,
        crate::api::routes::account::reset_password,
        crate::api::routes::account::profile,
        crate::api::routes::account::update_profile,
        crate::api::routes::account::change_password,
        crate::api::routes::account::request_email_change,
        crate::api::routes::account::confirm_email_change,
        crate::api::routes::account::sessions,
        crate::api::routes::account::revoke_session,
        crate::api::routes::account::revoke_all,
        crate::api::routes::account::export,
        crate::api::routes::account::delete_account,
        crate::api::routes::account::manage_list,
        crate::api::routes::account::provision,
        crate::api::routes::account::manage_lifecycle,
    ),
    components(
        schemas(
            crate::api::response::ErrorResponse,
            crate::api::response::StatusResponse,
        )
    )
)]
pub struct ApiDoc;

struct AccountApiContract;

impl utoipa::Modify for AccountApiContract {
    fn modify(&self, doc: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::{
            Required,
            path::{ParameterBuilder, ParameterIn},
            schema::{ObjectBuilder, Type},
            security::{ApiKey, ApiKeyValue, SecurityRequirement, SecurityScheme},
        };
        let components = doc.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "SessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Host-ag_session"))),
        );
        components.add_security_scheme(
            "LocalSessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("ag_session"))),
        );
        for (path, item) in &mut doc.paths.paths {
            if !path.starts_with("/api/v1/") {
                continue;
            }
            let requirements = if path.starts_with("/api/v1/management/")
                || path.starts_with("/api/v1/account")
                || path == "/api/v1/auth/logout"
            {
                Some(vec![
                    SecurityRequirement::new("SessionCookie", std::iter::empty::<String>()),
                    SecurityRequirement::new("LocalSessionCookie", std::iter::empty::<String>()),
                ])
            } else {
                None
            };
            for operation in [
                &mut item.get,
                &mut item.post,
                &mut item.patch,
                &mut item.delete,
                &mut item.put,
            ]
            .into_iter()
            .flatten()
            {
                operation.security = requirements.clone();
            }
            for operation in [
                &mut item.post,
                &mut item.patch,
                &mut item.delete,
                &mut item.put,
            ]
            .into_iter()
            .flatten()
            {
                operation.parameters.get_or_insert_with(Vec::new).push(ParameterBuilder::new().name("X-AsterGate-Request")
                    .parameter_in(ParameterIn::Header).required(Required::True)
                    .description(Some("Must be 1. Browser writes require a configured site Origin matching Host; cross-origin CORS is not enabled."))
                    .schema(Some(ObjectBuilder::new().schema_type(Type::String).enum_values(Some(["1"])))).build().into());
            }
        }
    }
}
