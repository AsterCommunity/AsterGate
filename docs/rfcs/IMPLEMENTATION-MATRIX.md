# RFC Implementation Matrix

This matrix tracks evidence from the current checkout and is intentionally conservative: a green
test for one vertical slice does not mark a broader protocol or product capability complete.

| Contract area | Current evidence | Status |
| --- | --- | --- |
| Account bootstrap and local password login | `tests/identity_flow.rs` bootstrap/login flows | Implemented slice |
| Organizations, memberships, groups and admission | identity integration tests and typed repositories | Implemented slice |
| Application `resource_id` binding | migration, DTO, typed client/resource repositories | Implemented slice |
| OIDC Authorization Code + PKCE S256 | identity flow, `oauth2` PKCE generation, OpenID metadata parsing | Implemented slice |
| Discovery, JWKS, UserInfo and logout | provider routes and identity flow assertions | Implemented slice |
| Refresh rotation, reuse detection and revocation | identity refresh integration flow | Implemented slice |
| Permission projection | typed group/role/resource projection; end-to-end group admission flow now covers denied admission, granted `read` scope, and access-token `permissions` projection | Implemented slice |
| Downstream plugin registration | connector registry unit tests | Implemented slice |
| OpenAPI and frontend generated types | `tests/generate_openapi.rs`, frontend generation/check | Implemented slice |
| Signing-key lifecycle | `signing_keys` schema, typed lifecycle repository, runtime rotation/reload, JWKS overlap, `kid` verification, operator rotate/retire/finalize/compromise APIs, and lifecycle integration tests | Implemented slice |
| Device authorization grant | typed persistence, start/approval endpoints, pending/slow-down/denied/expiry states, device-code token exchange, and single-use vertical flow are implemented; standards-level client conformance remains | Implemented slice |
| MFA, recovery codes and WebAuthn | Standard Base32 TOTP enrollment and `otpauth` URI, session-authenticated enrollment/confirmation HTTP APIs, encrypted secrets, confirmation counter consumption and session invalidation, browser-bound TOTP/recovery login, persisted account/challenge budgets, atomic counter/recovery/challenge consumption, recovery-set replacement, and router + SQLite replay/concurrency/rollback tests are implemented; factor management and security-setting step-up, email MFA, WebAuthn and passwordless methods remain | Partial |
| Upstream OIDC/OAuth/LDAP/SAML federation | organization-admin connector create/list/enable/disable/delete and encrypted client-secret rotation; redacted responses; enabled registered OIDC connectors support anonymous browser-bound start flows and authenticated link flows; state/browser/nonce/PKCE validation precedes atomic single-use callback claim; bounded HTTPS discovery/JWKS/token exchange with redirects disabled and streaming response limits; issuer-partitioned metadata cache; RS256 `kid`, issuer/audience/nonce/iat/exp/azp validation; linked identity must resolve to an active organization Membership. Router/SQLite covers connector secret redaction and lifecycle; repository/unit tests cover callback races/expiry and JWKS key selection. OIDC callback HTTP orchestration, account/session completion, RFC-driven upstream group mappings, LDAP/SAML drivers and external conformance remain | Partial |
| Key rotation overlap and retirement service | Transactional staged -> retiring -> active rotation, runtime reload, JWKS overlap publication, `kid`-selected verification, operator-protected rotate/retire/finalize/compromise API, and immediate compromised-key replacement are implemented | Implemented slice |
| PostgreSQL runtime acceptance | `ASTER_TEST_DATABASE_BACKEND=postgres cargo test --features openapi --test database_backends` | Passed |
| MySQL runtime acceptance | Clean random-name container run reaches AsterForge foundation migration and fails with MySQL error 1071 (`Specified key was too long; max key length is 3072 bytes`); AsterGate migration is not reached, and the failing upstream statement is not exposed by the foundation migration API | Blocked by upstream schema |
| Official OAuth/OIDC conformance suite | discovery metadata is tested with `oauth2`/`openidconnect`; no external conformance run artifact | Partial client-library coverage |

The repository must not advertise rows marked `Persistence only`, `Missing`, or `Unverified` as
complete product capabilities.
