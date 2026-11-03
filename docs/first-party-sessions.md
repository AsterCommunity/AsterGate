# First-Party Session Operations

This checkout implements [AG #5](https://github.com/AsterCommunity/AsterGate/issues/5) using
`aster_forge_auth` at the existing locked Forge revision `2483171`. It supplies Account and human
management authentication, not OAuth/OIDC issuance, JWKS, Organizations or tenant authorization.
The product adapter is `src/services/account/sessions.rs`; Forge owns JWT verification, issuance,
refresh coordination and classification. Gate owns Account authority, transactions and transport.

## Signing Configuration

```toml
[session]
issuer = "https://identity.example.com"
signing_key = "<64 hex characters generated independently with openssl rand -hex 32>"
```

First startup persists an independent random signing key and an issuer in `data/config.toml`
(`ASTER_CONFIG` overrides its location). Newly written keys use restrictive file permissions.
The development issuer is `http://localhost:3000`; production must configure a stable HTTPS issuer.
Issuer does not follow public site edits, primary mail site, Host, Origin or forwarded headers.
Credentials, query and fragment components are rejected; HTTP is allowed only on loopback.
Environment overrides are `ASTER__SESSION__ISSUER` and `ASTER__SESSION__SIGNING_KEY`. Injected keys
are not written back to the file. Invalid persisted keys fail startup rather than being regenerated.

Generate signing and AEAD master keys independently from a cryptographically secure random source.
Length validation is not an entropy test; a human-chosen 64-character value is not a suitable key.
Reusing `security.master_key` as the signing key is rejected. Back up both keys with the database;
share the same signing key and issuer across replicas. Secrets are excluded from config exports
and Debug. Signing configuration is static; product settings reload keeps the instance coordinator.

HS256 is pinned with a 32-byte key, zero clock leeway and Forge's 16 KiB token limit. Access uses
audience `aster-gate-web-access`; refresh uses `aster-gate-web-refresh`. Both require
`iss/aud/sub/sid/iat/exp/session_version/token_type/restriction`. Subject is the canonical Account
UUID. Access forbids JTI; refresh requires JTI. Only unrestricted local sessions are currently
issued; unsupported restrictions fail closed. JWTs carry no live permission list.

Key or issuer replacement invalidates existing credentials and requires sign-in. Coordinate the
replacement across replicas; there is no overlapping keyring or automatic key rotation in this slice.
Do not reuse this namespace/key for future downstream OAuth access tokens or OIDC ID tokens.

## Authority And Rotation

Every protected request verifies access and reads the writer Account plus owned `sid`. Account
must be active, Account/session security versions must match, and the session must be live and
unexpired. Permissions are read from current grants. Critical mutations obtain the Account writer
fence before rechecking the session and permissions. A cached JWT verdict cannot bypass revocation.

`session_version` is a separate security epoch. Profile edits and refresh do not advance it;
password reset/change, confirmed email replacement, all-session revoke, suspension, erasure and
confirmed reuse invalidate sessions atomically. Single-session revoke/logout affects only that sid.
Restore requires a new login. The immutable session deadline defaults to 24 hours. Access lasts at
most five minutes and never beyond that deadline; rotation does not slide or extend the deadline.

Login/setup issue within their existing writer transaction and deliver credentials only after commit.
Refresh follows this sequence:

1. Verify refresh purpose, signature and claims; enter the instance's bounded per-JTI coordinator.
2. Begin a writer transaction and lock the Account without advancing its business revision.
3. Read time again after acquiring the lock, then validate Account, epoch, restriction, owned sid,
   revocation, server deadline and the signed `exp + leeway` boundary.
4. Classify through Forge and conditionally replace the current JTI while preserving the deadline.
5. Commit before returning the new Cookie pair. A CAS conflict is reread in a new writer transaction
   and may only reject; it cannot issue another successor.

The policy is **Strict**: an older JTI on an otherwise valid owned session is confirmed reuse, even
with matching UA/IP or an immediate retry. Gate advances the Account epoch, revokes its first-party
sessions and writes `account.refresh_reused` in the same transaction, commits, then returns typed
401. Unrelated Accounts are untouched. Missing/foreign/revoked/expired sessions, mismatched epoch or
invalid claims reject without compromise side effects. Older generations remain detectable after
restart. UA/IP do not provide device identity or a successful-refresh grace window.

`auth.refresh_stale` is mapped for Forge's typed result but is not emitted by Strict classification.
Database, audit and commit failures are storage errors (503), never disguised as a committed reuse
rejection. Failed requests deliver no candidate tokens or clearing cookies. A lost successful refresh
response is not safely replayable under Strict; a manual replay can require fresh login on all devices.

The default coordinator budget includes running requests and waiters (1024 total). Exhaustion is
429; guards and lock entries are reclaimed when futures finish or are cancelled. Transaction drop
rolls back uncommitted changes. Runtime shutdown closes refresh admission, drains HTTP, then closes
the database. Coordinators belong to instances, not requests or a global product singleton.
Revocation does not depend on cache writes or invalidation notifications: these paths use the writer
directly. JTI/token/key material is excluded from Model/ActiveModel Debug and audit details; SQL
parameter logging is disabled on Forge connections.

## Cookie And Frontend Contract

| Purpose | HTTPS name | HTTP loopback name | Path | Attributes |
| --- | --- | --- | --- | --- |
| Access | `__Host-ag_access` | `ag_access` | `/` | HttpOnly, SameSite=Lax, host-only |
| Refresh | `__Secure-ag_refresh` | `ag_refresh` | `/api/v1/auth` | HttpOnly, SameSite=Lax, host-only |
| CSRF | `__Host-ag_csrf` | `ag_csrf` | `/` | Readable, SameSite=Strict, host-only |

HTTPS adds Secure. Cookie Max-Age is bounded by the corresponding signed deadline. Clearing matches
the original name/path/security attributes. There are no legacy `ag_session` aliases or Bearer
fallbacks. Each hostname has independent cookies and login state.

`POST /api/v1/auth/refresh` requires the refresh Cookie, `X-AsterGate-Request: 1`, matching CSRF
Cookie/header and the same source checks as other writes. It returns a boolean REST acknowledgement,
with credentials only in Set-Cookie. Setup/login rotate CSRF after commit; refresh preserves CSRF
to avoid rejecting concurrent writes. Logout accepts the current refresh even after access expires;
if refresh is absent, valid access alone can revoke its owned session. A predecessor cannot log out
the successor. Logout/security mutations clear access, refresh and CSRF only after revocation commits.

`sessionRefresh.ts` coordinates one promise per tab and a Web Lock across tabs. Login/setup/logout
use that same lock so a late refresh response cannot overwrite a new login's Cookie pair. Inside the lock it
checks `/account` first, so another tab's successful refresh is reused without replaying refresh.
Initial protected writes hold a shared lock through their response: writes can run concurrently, while
exclusive login/logout/refresh waits for their committed Cookie effects. Authentication rejections release
the shared lock before acquiring the exclusive recovery lock; ordinary reads do not acquire a lock.
The read-only `/account` probe has a ten-second timeout, releasing the lock on a stalled response.
Refresh is never automatically timed out or retried: a committed rotation with a lost response could
leave the browser holding a predecessor. Retried requests preserve the caller's cancellation signal.
Only Account/management requests rejected for missing/expired access qualify, with one retry.
Permission/password failures, public/auth routes, 429 and 503 do not trigger refresh. Lifecycle
notifications use BroadcastChannel without credentials. Each request snapshots the existing CSRF
Cookie as a synchronously readable identity fence: login/setup rotate it, logout clears it and refresh
preserves it. Protected requests check the snapshot after asynchronous request preparation, before
their first dispatch. Protected writes require a valid snapshot before preparation; a newly bootstrapped
marker cannot establish continuity with the identity that initiated the write.
Recovery checks it inside the lock, after the `/account` probe and refresh,
and before retry dispatch, including after asynchronous request preparation. Retries hold the same
lock through their response, so identity transitions cannot interleave with them. A missing or changed
CSRF Cookie aborts recovery with the original authentication error, independently of notification
delivery. JWTs never enter localStorage, sessionStorage, BroadcastChannel payloads or JavaScript-readable
cookies; the fence needs no additional browser storage.

Browsers without Web Locks safely require sign-in when access expires. Web Locks normally require
a secure context (HTTPS or loopback). Cross-origin hosts do not share login state or a refresh lock.
The first-party refresh route is distinct from the unimplemented OAuth `/token` endpoint.

## Development Schema

AsterGate is unreleased. The existing initial Account migration now defines Account security epoch
and refresh-generation fields on the owned browser-session aggregate; no parallel session store is
created. Pre-JWT development databases are incompatible with this schema. Back up existing data and
explicitly migrate it, or select a separate fresh development database. Never delete a user's database
implicitly or assume rerunning an already-recorded migration changes its existing tables.

## Verification Evidence

`tests/accounts/jwt_sessions.rs` exercises the actual router/writer: claims/algorithm/key/purpose/
time/size, absolute expiry, immediate revoke, Strict reuse, audit rollback, CAS/commit failure,
independent-instance and same-process contention, restart replay, key/issuer replacement, writer
wait across expiry, cancellation, coordinator budget/reclamation/shutdown and ORM redaction.
SQLite triggers inject CAS and deferred commit failures; applicable lifecycle and transaction tests
also run against PostgreSQL 16 and MySQL 8.4 with isolated Docker fixtures. `tests/platform/config_loader.rs`
verifies durable signing configuration and environment handling.

Frontend unit tests cover concurrent callers, another tab's completed refresh, one-retry limits,
typed failures, cookie changes without lifecycle notifications, initial/retry preparation, probe timeout
and missing Web Locks. The Chromium suite uses a real backend and two tabs to prove one shared refresh,
unchanged absolute expiry, immediate revocation, absence of stored credentials and rejection of an old
profile mutation after a second tab changes identity with BroadcastChannel unavailable.
Generated OpenAPI/SDK include refresh and both credential schemes.
No test in this slice establishes OAuth/OIDC conformance, MFA or tenant authorization.
