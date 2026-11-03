//! OpenAPI document assembly.
//!
//! Product routes should add their annotated handlers and DTO schemas here. The module is compiled
//! only for debug builds with the `openapi` feature, keeping normal release binaries free of API
//! documentation generation overhead.

use crate::api::{api_error_code::ApiErrorCode, dto::account::AccountDto, response::NoData};
use aster_forge_api::{CursorPage, response::ApiResponse};
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
        crate::api::routes::account::refresh,
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
            ApiResponse<NoData, ApiErrorCode, NoData>,
            crate::api::response::StatusResponse,
        )
    )
)]
pub struct ApiDoc;

struct AccountApiContract;

impl utoipa::Modify for AccountApiContract {
    /// Applies the shared REST error, Cookie and CSRF contract to the generated Account operations.
    /// Refresh requires its own credential scheme; logout also documents valid access-only revocation.
    fn modify(&self, doc: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::{
            RefOr, Required,
            content::ContentBuilder,
            header::HeaderBuilder,
            path::{ParameterBuilder, ParameterIn},
            response::ResponseBuilder,
            schema::{ObjectBuilder, Schema, Type},
            security::{ApiKey, ApiKeyValue, SecurityRequirement, SecurityScheme},
        };
        let components = doc.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "SessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Host-ag_access"))),
        );
        components.add_security_scheme(
            "LocalSessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("ag_access"))),
        );
        components.add_security_scheme(
            "CsrfCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Host-ag_csrf"))),
        );
        components.add_security_scheme(
            "LocalCsrfCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("ag_csrf"))),
        );
        components.add_security_scheme(
            "RefreshCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Secure-ag_refresh"))),
        );
        components.add_security_scheme(
            "LocalRefreshCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("ag_refresh"))),
        );
        let error_schema: RefOr<Schema> = utoipa::schema!(ApiResponse<
            NoData,
            ApiErrorCode,
            NoData
        >)
        .into();
        // Forge's envelope asks T::schema() for nested data. Utoipa's derived generic
        // CursorPage can project Option<C=String> as a dangling String reference; composing
        // the generic page also loses the Option wrapper. Supply this concrete nullable field
        // through the public schema macro, preserving the shared page and every other field.
        let account_page_schema: RefOr<Schema> =
            utoipa::schema!(ApiResponse<CursorPage<AccountDto, String>, ApiErrorCode, NoData>)
                .into();
        if let RefOr::Ref(reference) = account_page_schema
            && let Some(name) = reference.ref_location.strip_prefix("#/components/schemas/")
            && let Some(RefOr::T(Schema::Object(envelope))) = components.schemas.get_mut(name)
            && let Some(RefOr::T(Schema::Object(page))) = envelope.properties.get_mut("data")
        {
            page.properties
                .insert("next_cursor".into(), utoipa::schema!(Option<String>).into());
        }
        for (path, item) in &mut doc.paths.paths {
            if !path.starts_with("/api/v1/") {
                continue;
            }
            let refresh_auth = matches!(
                path.as_str(),
                "/api/v1/auth/refresh" | "/api/v1/auth/logout"
            );
            let cookie_scheme = if refresh_auth {
                "RefreshCookie"
            } else {
                "SessionCookie"
            };
            let local_cookie_scheme = if refresh_auth {
                "LocalRefreshCookie"
            } else {
                "LocalSessionCookie"
            };
            let requirements = if path.starts_with("/api/v1/management/")
                || path.starts_with("/api/v1/account")
                || refresh_auth
            {
                Some(vec![
                    SecurityRequirement::new(cookie_scheme, std::iter::empty::<String>()),
                    SecurityRequirement::new(local_cookie_scheme, std::iter::empty::<String>()),
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
                for (status, description) in [
                    ("400", "Invalid request"),
                    ("401", "Session required or credentials invalid"),
                    ("403", "Forbidden, including CSRF or origin rejection"),
                    ("404", "Resource not found"),
                    ("409", "Business state conflict"),
                    (
                        "429",
                        "Rate limited; Retry-After is advisory, never an automatic retry permit",
                    ),
                    ("503", "Service unavailable"),
                ] {
                    operation
                        .responses
                        .responses
                        .entry(status.into())
                        .or_insert_with(|| {
                            ResponseBuilder::new()
                                .description(description)
                                .content(
                                    "application/json",
                                    ContentBuilder::new()
                                        .schema(Some(error_schema.clone()))
                                        .build(),
                                )
                                .build()
                                .into()
                        });
                }
                for (status, response) in &mut operation.responses.responses {
                    let RefOr::T(response) = response else {
                        continue;
                    };
                    for (name, description) in [
                        ("Cache-Control", "no-store"),
                        ("Referrer-Policy", "no-referrer"),
                    ] {
                        response.headers.entry(name.into()).or_insert_with(|| {
                            HeaderBuilder::new()
                                .description(Some(description))
                                .schema(Some(ObjectBuilder::new().schema_type(Type::String)))
                                .build()
                                .into()
                        });
                    }
                    if status == "429" {
                        response.headers.insert("Retry-After".into(), HeaderBuilder::new()
                            .description(Some("Advisory delay in seconds or an HTTP-date; does not permit automatic replay"))
                            .schema(Some(ObjectBuilder::new().schema_type(Type::String)))
                            .build().into());
                    }
                }
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
                operation.security = Some(if requirements.is_some() {
                    vec![
                        SecurityRequirement::new(cookie_scheme, std::iter::empty::<String>())
                            .add("CsrfCookie", std::iter::empty::<String>()),
                        SecurityRequirement::new(local_cookie_scheme, std::iter::empty::<String>())
                            .add("LocalCsrfCookie", std::iter::empty::<String>()),
                    ]
                } else {
                    vec![
                        SecurityRequirement::new("CsrfCookie", std::iter::empty::<String>()),
                        SecurityRequirement::new("LocalCsrfCookie", std::iter::empty::<String>()),
                    ]
                });
                if path == "/api/v1/auth/logout" {
                    operation.security.get_or_insert_with(Vec::new).extend([
                        SecurityRequirement::new("SessionCookie", std::iter::empty::<String>())
                            .add("CsrfCookie", std::iter::empty::<String>()),
                        SecurityRequirement::new(
                            "LocalSessionCookie",
                            std::iter::empty::<String>(),
                        )
                        .add("LocalCsrfCookie", std::iter::empty::<String>()),
                    ]);
                }
                operation.parameters.get_or_insert_with(Vec::new).push(ParameterBuilder::new().name("X-AsterGate-Request")
                    .parameter_in(ParameterIn::Header).required(Required::True)
                    .description(Some("Must be 1. Browser writes require a configured same-origin site; CORS allowlisting does not bypass this requirement."))
                    .schema(Some(ObjectBuilder::new().schema_type(Type::String).enum_values(Some(["1"])))).build().into());
                operation.parameters.get_or_insert_with(Vec::new).push(ParameterBuilder::new()
                    .name("X-AsterGate-CSRF").parameter_in(ParameterIn::Header).required(Required::True)
                    .description(Some("Echo the readable __Host-ag_csrf (HTTPS) or ag_csrf (local HTTP) cookie. Bootstrap it with GET /api/v1/setup/status or /api/v1/public/frontend-config. Setup/login rotate CSRF; refresh preserves it. Required for every unsafe REST request."))
                    .schema(Some(ObjectBuilder::new().schema_type(Type::String))).build().into());
            }
        }
    }
}
