use crate::{
    api::{
        account_security::{self, Authenticated, JsonInput, SessionCookiePolicy},
        dto::account::*,
    },
    runtime::AppState,
    services::account::{self, AccountError, Result, models::*},
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/setup/status", get(super::system::status))
        .nest("/public", super::public::router())
        .route("/setup", post(super::system::initialize))
        .route(
            "/management/settings",
            get(super::system::settings).put(super::system::update_settings),
        )
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/verification", post(request_verification))
        .route("/auth/verify-email", post(verify_email))
        .route("/auth/recovery", post(request_recovery))
        .route("/auth/reset-password", post(reset_password))
        .route(
            "/account",
            get(profile).patch(update_profile).delete(delete_account),
        )
        .route("/account/password", post(change_password))
        .route("/account/email", post(request_email_change))
        .route("/account/email/confirm", post(confirm_email_change))
        .route("/account/sessions", get(sessions).delete(revoke_all))
        .route("/account/sessions/{id}", delete(revoke_session))
        .route("/account/export", get(export))
        .route("/management/accounts", get(manage_list).post(provision))
        .route(
            "/management/accounts/{id}/lifecycle",
            post(manage_lifecycle),
        )
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(
            state,
            account_security::boundary,
        ))
}

fn success() -> Json<SuccessResponse> {
    Json(SuccessResponse::ok(true))
}
fn clear(policy: &SessionCookiePolicy) -> Result<Response> {
    let mut response = success().into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        account_security::session_cookie(policy, "")?,
    );
    Ok(response)
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/auth/register", tag = "account", request_body = RegisterRequest, responses((status = 201, body = AccountResponse), (status = 409, body = crate::api::response::ErrorResponse)))]
pub async fn register(
    State(state): State<Arc<AppState>>,
    JsonInput(input): JsonInput<RegisterRequest>,
) -> Result<impl IntoResponse> {
    Ok((
        StatusCode::CREATED,
        Json(AccountResponse::ok(
            account::register(&state, input.try_into()?).await?.into(),
        )),
    ))
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/auth/login", tag = "account", request_body = LoginRequest, responses((status = 200, body = AccountResponse), (status = 401, body = crate::api::response::ErrorResponse)))]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Extension(cookie_policy): Extension<SessionCookiePolicy>,
    headers: HeaderMap,
    JsonInput(input): JsonInput<LoginRequest>,
) -> Result<Response> {
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let result = account::login(&state, &input.identifier, input.password, ua).await?;
    let mut response = Json(AccountResponse::ok(result.account.into())).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        account_security::session_cookie(&cookie_policy, &result.session_token)?,
    );
    Ok(response)
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/auth/logout", tag = "account", responses((status = 200, body = SuccessResponse)))]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    Extension(cookie_policy): Extension<SessionCookiePolicy>,
    Authenticated(principal): Authenticated,
) -> Result<Response> {
    account::revoke_session(
        &state,
        &principal,
        &SessionId(principal.session_id.0.clone()),
    )
    .await?;
    clear(&cookie_policy)
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/auth/verification", tag = "account", request_body = IdentifierRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn request_verification(
    State(state): State<Arc<AppState>>,
    JsonInput(input): JsonInput<IdentifierRequest>,
) -> Result<Json<SuccessResponse>> {
    account::request_challenge(&state, &input.identifier, ChallengePurpose::VerifyEmail).await?;
    Ok(success())
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/auth/verify-email", tag = "account", request_body = TokenRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn verify_email(
    State(state): State<Arc<AppState>>,
    JsonInput(input): JsonInput<TokenRequest>,
) -> Result<Json<SuccessResponse>> {
    account::verify_email(&state, &input.token).await?;
    Ok(success())
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/auth/recovery", tag = "account", request_body = IdentifierRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn request_recovery(
    State(state): State<Arc<AppState>>,
    JsonInput(input): JsonInput<IdentifierRequest>,
) -> Result<Json<SuccessResponse>> {
    account::request_challenge(&state, &input.identifier, ChallengePurpose::ResetPassword).await?;
    Ok(success())
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/auth/reset-password", tag = "account", request_body = ResetPasswordRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn reset_password(
    State(state): State<Arc<AppState>>,
    JsonInput(input): JsonInput<ResetPasswordRequest>,
) -> Result<Json<SuccessResponse>> {
    account::reset_password(&state, &input.token, input.password).await?;
    Ok(success())
}

#[aster_forge_api_docs_macros::path(get, path = "/api/v1/account", tag = "account", responses((status = 200, body = AccountResponse)))]
pub async fn profile(
    State(state): State<Arc<AppState>>,
    Authenticated(principal): Authenticated,
) -> Result<Json<AccountResponse>> {
    Ok(Json(AccountResponse::ok(
        account::profile(&state, &principal).await?.into(),
    )))
}

#[aster_forge_api_docs_macros::path(patch, path = "/api/v1/account", tag = "account", request_body = ProfileRequest, responses((status = 200, body = AccountResponse)))]
pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    Authenticated(principal): Authenticated,
    JsonInput(input): JsonInput<ProfileRequest>,
) -> Result<Json<AccountResponse>> {
    Ok(Json(AccountResponse::ok(
        account::update_profile(&state, &principal, &input.display_name)
            .await?
            .into(),
    )))
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/account/password", tag = "account", request_body = PasswordRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    Extension(cookie_policy): Extension<SessionCookiePolicy>,
    Authenticated(principal): Authenticated,
    JsonInput(input): JsonInput<PasswordRequest>,
) -> Result<Response> {
    account::change_password(
        &state,
        &principal,
        input.current_password,
        input.new_password,
    )
    .await?;
    clear(&cookie_policy)
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/account/email", tag = "account", request_body = EmailChangeRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn request_email_change(
    State(state): State<Arc<AppState>>,
    Authenticated(principal): Authenticated,
    JsonInput(input): JsonInput<EmailChangeRequest>,
) -> Result<Json<SuccessResponse>> {
    account::request_email_change(&state, &principal, &input.email, input.password).await?;
    Ok(success())
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/account/email/confirm", tag = "account", request_body = TokenRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn confirm_email_change(
    State(state): State<Arc<AppState>>,
    Extension(cookie_policy): Extension<SessionCookiePolicy>,
    Authenticated(principal): Authenticated,
    JsonInput(input): JsonInput<TokenRequest>,
) -> Result<Response> {
    account::confirm_email_change(&state, &principal, &input.token).await?;
    clear(&cookie_policy)
}

#[aster_forge_api_docs_macros::path(get, path = "/api/v1/account/sessions", tag = "account", responses((status = 200, body = SessionsResponse)))]
pub async fn sessions(
    State(state): State<Arc<AppState>>,
    Authenticated(principal): Authenticated,
) -> Result<Json<SessionsResponse>> {
    Ok(Json(SessionsResponse::ok(
        account::list_sessions(&state, &principal)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    )))
}

#[aster_forge_api_docs_macros::path(delete, path = "/api/v1/account/sessions/{id}", tag = "account", params(("id" = String, Path)), responses((status = 200, body = SuccessResponse)))]
pub async fn revoke_session(
    State(state): State<Arc<AppState>>,
    Extension(cookie_policy): Extension<SessionCookiePolicy>,
    Authenticated(principal): Authenticated,
    Path(id): Path<String>,
) -> Result<Response> {
    let current = principal.session_id.0 == id;
    account::revoke_session(&state, &principal, &SessionId::parse(&id)?).await?;
    if current {
        clear(&cookie_policy)
    } else {
        Ok(success().into_response())
    }
}

#[aster_forge_api_docs_macros::path(delete, path = "/api/v1/account/sessions", tag = "account", responses((status = 200, body = SuccessResponse)))]
pub async fn revoke_all(
    State(state): State<Arc<AppState>>,
    Extension(cookie_policy): Extension<SessionCookiePolicy>,
    Authenticated(principal): Authenticated,
) -> Result<Response> {
    account::revoke_all_sessions(&state, &principal).await?;
    clear(&cookie_policy)
}

#[aster_forge_api_docs_macros::path(get, path = "/api/v1/account/export", tag = "account", responses((status = 200, body = ExportResponse)))]
pub async fn export(
    State(state): State<Arc<AppState>>,
    Authenticated(principal): Authenticated,
) -> Result<Json<ExportResponse>> {
    Ok(Json(ExportResponse::ok(AccountExportDto {
        account: account::profile(&state, &principal).await?.into(),
        sessions: account::list_sessions(&state, &principal)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    })))
}

#[aster_forge_api_docs_macros::path(delete, path = "/api/v1/account", tag = "account", request_body = DeleteAccountRequest, responses((status = 200, body = SuccessResponse)))]
pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    Extension(cookie_policy): Extension<SessionCookiePolicy>,
    Authenticated(principal): Authenticated,
    JsonInput(input): JsonInput<DeleteAccountRequest>,
) -> Result<Response> {
    account::delete_self(&state, &principal, input.password).await?;
    clear(&cookie_policy)
}

#[aster_forge_api_docs_macros::path(get, path = "/api/v1/management/accounts", tag = "account-management", params(("after" = Option<String>, Query), ("limit" = Option<u64>, Query)), responses((status = 200, body = AccountsResponse)))]
pub async fn manage_list(
    State(state): State<Arc<AppState>>,
    Authenticated(operator): Authenticated,
    query: std::result::Result<Query<AccountListQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<AccountsResponse>> {
    let Query(query) = query.map_err(|_| AccountError::InvalidInput)?;
    let after = query.after.as_deref().map(AccountId::parse).transpose()?;
    let limit = query.limit.unwrap_or(50);
    let data: Vec<AccountDto> = account::manage_list(&state, &operator, after.as_ref(), limit)
        .await?
        .into_iter()
        .map(Into::into)
        .collect();
    let next_cursor = if u64::try_from(data.len()).map_err(|_| AccountError::Storage)? == limit {
        data.last().map(|a| a.id.clone())
    } else {
        None
    };
    let total = crate::db::repository::account_repo::count(state.db_handles.writer()).await?;
    Ok(Json(AccountsResponse::ok(
        aster_forge_api::CursorPage::new(data, total, limit, next_cursor),
    )))
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/management/accounts", tag = "account-management", request_body = RegisterRequest, responses((status = 201, body = AccountResponse)))]
pub async fn provision(
    State(state): State<Arc<AppState>>,
    Authenticated(operator): Authenticated,
    JsonInput(input): JsonInput<RegisterRequest>,
) -> Result<impl IntoResponse> {
    Ok((
        StatusCode::CREATED,
        Json(AccountResponse::ok(
            account::provision(&state, &operator, input.try_into()?)
                .await?
                .into(),
        )),
    ))
}

#[aster_forge_api_docs_macros::path(post, path = "/api/v1/management/accounts/{id}/lifecycle", tag = "account-management", params(("id" = String, Path)), request_body = LifecycleRequest, responses((status = 200, body = AccountResponse)))]
pub async fn manage_lifecycle(
    State(state): State<Arc<AppState>>,
    Authenticated(operator): Authenticated,
    Path(id): Path<String>,
    JsonInput(input): JsonInput<LifecycleRequest>,
) -> Result<Json<AccountResponse>> {
    Ok(Json(AccountResponse::ok(
        account::manage_lifecycle(
            &state,
            &operator,
            &AccountId::parse(&id)?,
            input.action.into(),
            input.current_password,
        )
        .await?
        .into(),
    )))
}
