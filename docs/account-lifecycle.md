# Account Lifecycle Contract

## Design And Implementation Status

The accepted authentication architecture is AD-style access/refresh JWTs, durable browser sessions,
Account session versions, refresh rotation and replay handling. It is defined in
[RFC 0009](rfcs/0009-first-party-jwt-sessions.md), with shared mechanisms tracked by
[AF #42](https://github.com/AsterCommunity/AsterForge/issues/42).
Account/setup delivery is tracked by [AG #2](https://github.com/AsterCommunity/AsterGate/issues/2),
JWT adoption by [AG #5](https://github.com/AsterCommunity/AsterGate/issues/5), shared HTTP protection
by [AG #4](https://github.com/AsterCommunity/AsterGate/issues/4), and audit/delegation by
[AG #12](https://github.com/AsterCommunity/AsterGate/issues/12).

JWT adoption is pending. The current checkout and completed lifecycle tests still use a single
opaque session Cookie. Sections describing existing routes, schema and tests record that implementation;
the JWT sections below define its replacement. They do not advertise a refresh endpoint or JWT support
before the code, schema, generated API and browser behavior have passed their acceptance gates.

## Ownership

Account is global to one instance. Account, identifiers, password credentials, browser sessions,
and mail challenges have no Organization column. The current module creates no Membership, Group,
Application admission, downstream OAuth grant/token, signing key, or external identity. Its accepted
next session design adds first-party JWT credentials, not downstream protocol issuance. An Account operator
uses explicit [system permissions](system-authorization.md), not an `is_admin` bit or a separate management token.
Organization administration and scoped role permissions remain unimplemented.

DTOs in `src/api/dto/account.rs` own Serde/OpenAPI. Service commands/results and Principal use
domain IDs, lifecycle/action enums and explicit fields. Services own policy and transactions;
Repositories own queries and conditional writes. Architecture tests enforce these boundaries.

## Implemented State Machine

| Event | Required state | Result |
| --- | --- | --- |
| Register or operator provision | New identity, unique normalized identifiers | Active/unverified by default; pending if verification is required; mail queued when SMTP is configured |
| Verify email | Active/unverified or pending, matching unexpired unused verification challenge | Active with verified email; link consumed |
| Sign in | Active, correct password | Fresh random opaque browser session |
| Suspend | Active or pending | Suspended; original state retained; all sessions and challenges revoked |
| Restore | Suspended | Original active/pending state restored; no revoked session or challenge resurrects |
| Delete | Any non-deleted state, exact system permission and operator password or current password for self-service | Terminal deleted tombstone; profile, identifiers, credentials, sessions and challenges erased |

Deleted IDs are never reused. A released username/email may be registered as an entirely new
Account with a new random ID. Deletion leaves redacted audit facts and the UUID tombstone, but no
profile, login identifier, credential or browser-session data. Account-associated mail recipients
and payloads are erased, including previously delivered messages in the local outbox. This does
not delete messages already delivered to a recipient's external mailbox.

## Credentials And Current Sessions

- Usernames normalize to lowercase; 3–32 ASCII letters/digits/underscore/hyphen. Emails use Forge's
  email normalization/validation. Database uniqueness resolves concurrent registration, never
  a preflight lookup alone. Display names are nonempty, at most 128 bytes and contain no controls.
- Passwords are 12–128 bytes, untrimmed, stored as salted Argon2id PHC hashes using Forge's bounded
  RFC 9106 policy. Blocking hash work is limited to four concurrent operations per process.
- Sessions are random 256-bit opaque values; only SHA-256 digests are stored. No browser bearer
  token is returned in JSON or stored in localStorage. Sessions have fixed configurable expiration
  (default 24 hours) and at most 20 live sessions per Account. This is today's transport, replaced
  by the accepted JWT design below without an old-format compatibility path.
- Every authentication check reads the writer database and requires active Account plus live,
  unrevoked session. Credential/status mutation and session creation claim the Account revision
  inside the transaction, so stale password checks cannot create a session after revocation.
- Changing/resetting a password or confirming a replacement email revokes every session and
  pending challenge. A password change must differ from the current password. Reset does not
  authenticate automatically. Account deletion requires current-password proof for self-service.
- An Account may inspect/revoke only its own sessions. Unknown and foreign session IDs return the
  same not-found result. Revoking all sessions includes the current session.

## Accepted JWT Session Design

### Credentials And Durable Authority

The hosted Account and human management interfaces use short-lived access JWTs and refresh JWTs,
following AD's proven session architecture. Both require explicit `iss`, `aud`, `sub`, `sid`, `jti`,
`iat`, `exp`, token type and session version. Access and refresh use distinct validation purposes;
a refresh JWT never authenticates an Account/management request. First-party `sub` identifies the
Account UUID, while downstream OIDC retains its public/pairwise subject mapping.

Issuer is explicit stable instance configuration. Editable site URLs and the primary email site do
not change it. Signing policy pins trusted keys and allowed algorithms; session signing material is
separate from the AEAD `master_key`. Secrets and complete token strings never enter logs, ordinary
storage rows, exported profile data or audit detail. The internal signing policy is completed with
AF #42; downstream signing/JWKS remains its own RFC capability.

Account `session_version` is a security epoch, not the ordinary Account mutation `revision`. Browser
sessions retain Account ownership, absolute expiry, revocation state, current refresh JTI/generation,
bounded predecessor/replay evidence and presentation metadata. Access defaults to a short lifetime;
refresh renewal stays within the configured session expiry rather than extending it indefinitely.
Login persists the session before exposing either token or Set-Cookie headers.

### Authentication And Permissions

Access verification checks signature, issuer, exact first-party audience, token type, required bounded
claims and times. It then checks the active Account, current security version and live owned `sid`
against authoritative state. A revoked session denies an unexpired access JWT immediately. JWT
validation cannot skip the session lookup or use a stale cache to relax the existing revocation promise.

AD currently checks access JWTs primarily through Account status/version without a single-session
`sid`; AG preserves its stronger per-session boundary by requiring that additional binding.
`GET /api/v1/account` still returns current `system_permissions` for frontend controls. Service use
cases check exact live grants; claims and button visibility do not authorize. Destructive lifecycle
and settings operations retain current-password proof, transaction rechecks and last-holder protection.
Changing permissions does not require waiting for a JWT to expire.

### Refresh, Replay And Revocation

The refresh endpoint verifies the refresh-specific policy, Account/version and owned active session,
then conditionally claims its current JTI/generation under the product transaction. The DB CAS is
the multi-replica authority. A bounded per-JTI lock can identify a same-process race, but cannot replace
the conditional write. Tokens are returned only after a successful commit.

| Event | JWT target behavior |
| --- | --- |
| Fresh login | Commit browser session and initial refresh generation; issue access/refresh pair |
| Valid current refresh | One committed successor generation and token pair; previous refresh never succeeds again |
| Recognized stale retry/concurrent loser | Typed rejection without new tokens or a second successful rotation |
| Confirmed refresh reuse | Reject, advance Account session version and revoke first-party sessions with required audit facts |
| Single-session revoke/logout | Mark `sid` inactive; immediately reject its access and refresh tokens |
| All-session logout, credential/security mutation | Atomically invalidate the applicable sessions and security epoch; do not change unrelated tenant Memberships |
| Expired, suspended or deleted Account/session | Reject without renewal, rollback bypass or resurrection |
| Restore Account | New login required; prior JWT versions and refresh generations remain invalid |
| Storage/audit/commit failure | No success or usable uncommitted tokens; committed reuse revocation is not rolled back by the rejection response |

Stale retry requires explicit evidence and always remains unsuccessful; there is no generic grace
period accepting old refresh credentials. AD's previous-JTI/recent-client classification is a
reference with testable policy boundaries. UA/IP are spoofable retry signals, not identity proofs.
Older generations, cross-session/Account inputs and replay after restart remain covered. Post-commit
cache/notification failure is distinct from transaction rollback and cannot restore a revoked session.

Global first-party compromise handling is separate from downstream OAuth refresh-family revocation.
An internal JWT is not accepted as a downstream resource token, OIDC ID token or OAuth refresh token.

### Browser And Frontend Integration

Use separate HttpOnly access/refresh Cookies. Access uses Path `/`; refresh is scoped to the
authentication routes, with matching attributes when clearing. Production Cookies are Secure,
SameSite=Lax and host-only; use `__Secure-` for a restricted refresh Path, because `__Host-` requires
Path `/`. Keep exact Host/Origin/custom-header CSRF validation and independent multi-site logins.
Neither token enters localStorage or browser-readable persistent storage.

The frontend coordinates refresh within each tab and retries an eligible request at most once.
Permission denial and password-proof failure do not trigger refresh. Cross-tab losers cannot overwrite
new credentials or loop indefinitely; typed stale/reuse/expired outcomes drive bounded recovery or
sign-in. Login/refresh token delivery is via Set-Cookie, not JSON token exposure. A future Bearer
profile requires its own explicit transport/ambiguity rules and is not automatically enabled.

### Forge And Product Ownership

AF #42 owns reusable typed JWT verification/issuance, refresh rotation outcomes and storage-port
contracts with injected clocks/IDs and resource limits. AG owns Account state and UUIDs, session
records/versions, live permissions, signing policy, Cookie/CSRF, repositories, transactions, frontend
and API error mapping. AF returns typed security facts; AG atomically persists necessary revocation
and audit. `system_permission_granted` and grant/revoke attribution belong in durable audit logs;
that audit work remains the separate follow-up, not a completed JWT prerequisite.

The JWT implementation updates the existing owned session aggregate and initial migration under
the unreleased development policy; it does not create parallel session persistence or opaque/JWT
fallback validation. Existing user databases are never silently reset.

## Email Flows

Setup creates the first Account with explicit system permissions in one transaction. Open registration can
create an active Account without SMTP, with its email marked unverified. Requiring email verification
is a separate policy and requires SMTP. Unverified email never proves identity and cannot be used for
recovery or automatic linking. Startup validates deployment cryptography; setup stores product
settings in `system_config`.

Every challenge records Account, purpose, target email, token digest, expiration and consumption
state. The mail outbox contains an AES-256-GCM envelope authenticated to the challenge ID, never
the plaintext token. Outbox creation, challenge creation and Account mutation share a transaction.
Resending invalidates previous links and cancels their queued payloads. Expired mail is discarded
before delivery; terminal delivery scrubs encrypted payloads. Metadata and mail recipients are
retired within 24 hours after challenge expiry. Database, audit, or outbox failure rolls back all
business mutations; failed SMTP remains queued with bounded retry and a terminal failure state.

Recovery/verification requests return the same success response for unknown, suspended, deleted
and ineligible Accounts. Recovery cannot activate a pending Account or restore a suspension.
Links are purpose-specific, expire (default 30 minutes), and can be consumed only once. Concurrent
consumption has one winner. An email replacement keeps the old verified email until confirmation,
requires current-password proof to request, and requires the requesting Account's session to
confirm. A uniqueness failure leaves the old email and unconsumed challenge intact.

Mail links store tokens in the URL fragment. Hosted pages remove the fragment from browser history
on mount and require an explicit POST confirmation; opening/prefetching a link performs no mutation.
A replacement link opened in a fresh browser prompts for the original Account login without
persisting the token. If the page is reloaded after fragment removal, request a new link.

## HTTP Security

Current production cookies are Secure, HttpOnly, SameSite=Lax, Path=/, host-only and use the `__Host-`
prefix. Local non-Secure cookies require loopback HTTP. Duplicate session cookies are rejected.
All Account/auth/management responses use `Cache-Control: no-store` and `Referrer-Policy:
no-referrer`. Request bodies are bounded to 16 KiB; unknown DTO/query fields and malformed JSON
are rejected rather than silently accepted.

Every write requires `X-AsterGate-Request: 1`. Browser writes require an exact configured site
Origin matching the configured request Host when an Origin header is present. No cross-origin CORS grant is provided. A foreign
browser cannot send the required custom header without a rejected preflight. CLI writes can omit
Origin but must still send the header and the applicable credential.
Mutable branding and ordered sites, revision checks and configuration synchronization are defined in
[Runtime configuration](runtime-configuration.md).
The current custom-header/source checks are not an implementation of AD's double-submit middleware.
The target [HTTP middleware integration](http-middleware.md) adopts Forge's shared Axum mechanisms
when AF #41 is published and verified, retaining AG's source, Cookie and permission boundaries.

Management APIs use an active Account session and exact `astergate.system.*` operation permissions.
The seven permission definitions, authentication requirements, last-holder protection and tests
are specified in [System authorization](system-authorization.md). Human administrator MFA and
role delegation remain outside this module.

Persisted attempt budgets are reserved before expensive hashing and survive failed requests and
restarts. Each 15-minute window allows 20 login attempts per normalized identifier and per Account
(aliases share the Account budget), 1,000 total login attempts, 5 registrations per email/100 total,
3 challenge requests per identifier/100 total, 3 email-change requests per Account, and 10 password
step-up/reset attempts per Account. A 429 includes `Retry-After: 900`. Budgets are HMAC-keyed using the deployment master key; they contain no clear login identifiers. These bounded limits are the
initial product policy, not a substitute for deployment ingress capacity controls.

## API Surface

### Implemented REST Routes

| API | Authentication | Behavior |
| --- | --- | --- |
| `GET /api/v1/public/frontend-config` | Public | Branding, setup state, registration/mail availability and password minimum |
| `POST /api/v1/auth/register` | Public, registration enabled | Active/unverified or pending by policy; mail when configured |
| `POST /api/v1/auth/login`, `/auth/logout` | Password / Account session | Create/revoke current browser session |
| `POST /api/v1/auth/verification`, `/auth/verify-email` | Public challenge | Request/consume activation |
| `POST /api/v1/auth/recovery`, `/auth/reset-password` | Public challenge | Request/consume password recovery |
| `GET/PATCH/DELETE /api/v1/account` | Account session; password for delete | Read/edit profile, erase Account |
| `POST /api/v1/account/password` | Account session + current password | Change password and revoke all sessions |
| `POST /api/v1/account/email`, `/account/email/confirm` | Account session; password to request | Verify replacement email |
| `GET/DELETE /api/v1/account/sessions` | Account session | List/revoke all owned sessions |
| `DELETE /api/v1/account/sessions/{id}` | Account session | Revoke owned session |
| `GET /api/v1/account/export` | Account session | Export current profile/active-session metadata, no secrets |
| `GET/POST /api/v1/management/accounts` | Account operator | Cursor list (1–100/page), provision pending Account |
| `POST /api/v1/management/accounts/{id}/lifecycle` | Account operator | Suspend, restore, terminal erase |

Management and Account REST responses use `{ code: "success", msg: "", data }`. Error responses have stable `code`,
`msg`, and `error.retryable` fields. These are product REST APIs, not OAuth/OIDC protocol endpoints.
The generated OpenAPI records session/operator schemes and the required write header.
The proposed first-party refresh route and JWT-specific errors/responses are added with the
implementation; this table and current generated SDK do not claim those endpoints already exist.

## Acceptance

### Current Evidence

`tests/accounts/` executes the real Router against SQLite by default, or an isolated
PostgreSQL/MySQL container using `ASTER_TEST_DATABASE_BACKEND`. It covers normal lifecycle,
identifier races, challenge races/replay/expiry/purpose, Account ownership, operator separation,
CSRF, malformed/oversized/unknown fields, session expiry/revocation, failed audit/session/outbox
writes and rollback, SMTP failure/retry, pending-state restoration, erasure, and cleanup.

`tests/accounts/architecture.rs`, `tests/platform/account_configuration.rs`, and migration/OpenAPI tests
cover module/schema/configuration/generation contracts. Playwright uses a real Rust process and
local SMTP server for registration/activation, profile/session failure recovery, password change,
password recovery, replacement email in a fresh browser, deletion and operator lifecycle. Desktop
and mobile screenshots and horizontal-overflow assertions verify the actual account surface.

### JWT Adoption Gates

RFC 0009 additionally requires strict token-type/key/algorithm/issuer/audience/claims/time/size checks,
single/global immediate revocation, unchanged precise permissions, one-winner same-process/cross-tab/
multi-replica refresh, replay after restart, stale/reuse classification, transaction/audit/commit/cache
failure behavior, token redaction and bounded lock/worker lifetimes. Real-router and browser tests
must prove Cookie issuance/clearing, CSRF and frontend refresh recovery. Generated contracts and
necessary database matrices must pass before the matrix marks JWT sessions Implemented.
Current opaque-session results prove none of the unimplemented JWT/refresh-specific behavior.

No test here proves Organization authorization, upstream federation, OAuth/OIDC conformance,
production SMTP deliverability, hosted mail reputation, MFA, or remote deployment. Those retain
separate implementation and acceptance gates in the RFC matrix.
