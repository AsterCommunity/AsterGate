# REST response contract

AsterGate consumes the shared controlled REST response from aster_forge_api at Forge revision
2483171e4c7201e64e8f3a9dcecbd9b4cc86235b. The Git dependency and Cargo.lock must agree.

## Migration ownership

| Previous Gate type/module | Shared API | Gate responsibility | Evidence |
| --- | --- | --- | --- |
| response! DTO wrappers (AccountResponse, AccountsResponse, SessionsResponse, ExportResponse, SetupStatusResponse, SettingsResponse, PublicFrontendConfigResponse, SuccessResponse) | ApiResponse<T, ApiErrorCode>::ok | DTO conversion, success code, existing data:true acknowledgements | account/settings vertical flows; response_envelope; generated SDK operation types |
| response::ErrorResponse, local ApiErrorInfo | ApiResponse::error_with_details, ApiErrorInfo | ApiErrorCode, public message, retryability, diagnostic disclosure | platform::error_codes; response_envelope; panel error tests |
| AccountError::IntoResponse, fallback adapter | Shared constructors plus Axum IntoResponse | HTTP status, Retry-After, session/CSRF Set-Cookie, no-store/no-referrer, source redaction | actual Gate router test; account security tests |
| Frontend concrete wrapper names | OperationJsonResponse, OperationData | Runtime validation, UI/i18n, authentication-expiry policy, optional advisory retry metadata | accountService/httpClient/API helper tests; generated typecheck |

Gate does not copy envelope serialization or lifecycle validation. response::NoData is a named unit
alias for the OpenAPI attribute parser, not another wire type or wrapper. Generic component names
retain data, code and diagnostic parameters, including explicit unit diagnostics in route schemas; tests resolve every generated reference and verify that
distinct DTO schemas remain distinct. No Axum or Actix adapter moves into Forge's transport-free core.

The current Forge envelope asks T::schema() for nested DTOs. Utoipa's derived CursorPage can
project its primitive String cursor as an unresolved component when that call omits composition
arguments. Composing the generic page also loses the nullable Option wrapper. The Gate OpenAPI modifier
uses Utoipa's public schema macro with the concrete Option<String> cursor field. Runtime pagination stays Forge-owned; no duplicate page DTO,
String pseudo-component or weakened reference check is introduced. Remove this projection adapter
when upstream nested generic composition is fixed, retaining nullable string cursor regression tests.

## Wire and security boundaries

- Success is {code:"success",msg,data?} and never contains error; failure has a Gate code and
  message, never data, and can contain {error:{retryable,diagnostic?}}.
- Absent data, explicit null, and {} remain distinct shared representations. Existing product
  operations keep their original DTO or boolean data; the panel validates that these operations return
  data before exposing it as required to callers. It does not normalize null or invent defaults.
- Gate's public failure adapter accepts no diagnostic argument. Internal database, key material,
  password, session or challenge details cannot escape through an arbitrary serialized error source.
  The optional typed diagnostic contract is exercised on an isolated synthetic Router only.
- Retryability is true only for rate_limited and service.unavailable. It is metadata, not permission
  to replay requests. The panel retains HTTP status and reads delta-seconds or HTTP-date Retry-After
  without automatic retries. Only HTTP 401 plus auth.session_required implies sign-in is required;
  auth.credentials_failed remains a failed login, not an expired existing session.
- Account/management HTTP status, Cache-Control:no-store, Referrer-Policy:no-referrer, session
  cookies and rate-limit headers remain Gate policy. Unsafe browser REST calls echo the readable CSRF
  cookie into X-AsterGate-CSRF; GET setup/status or public/frontend-config bootstraps the cookie.
- Health/readiness, metrics and OAuth/OIDC/JWKS/SAML protocol endpoints retain their own contracts.
  A successful HTTP response with malformed JSON or an invalid REST envelope fails at the panel
  boundary. Non-JSON proxy failures retain their HTTP status without inventing a product error code.

## Generation and verification

Generate the source of truth with cargo test --features openapi --test generate_openapi, then run
cd frontend-panel && bun run generate-api. Both generated OpenAPI and TypeScript are checked in.
Run Rust default/openapi/metrics tests and Clippy, and panel check/test/build. Capability status must
be based on the commands actually completed; this contract does not substitute for test evidence.
