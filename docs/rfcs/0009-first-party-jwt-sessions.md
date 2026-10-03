# RFC 0009: First-Party JWT Sessions And Forge Ownership

- Status: Accepted
- Date: 2026-10-04
- Owners: AsterGate maintainers
- Depends on: RFC 0001, RFC 0002, RFC 0003, RFC 0007
- Amends: RFC 0001, RFC 0002, RFC 0003, RFC 0005
- Shared mechanism: [AsterForge #42](https://github.com/AsterCommunity/AsterForge/issues/42)
- Product integration: [AG #5](https://github.com/AsterCommunity/AsterGate/issues/5)
- HTTP security integration: [AG #4](https://github.com/AsterCommunity/AsterGate/issues/4), dependent on AF #41
- Implementation: Pending; the current Account implementation still uses opaque session cookies

## Decision

AsterGate adopts AsterDrive's access/refresh JWT architecture for first-party Account and human
management authentication: short-lived access JWTs, refresh JWTs, durable sessions, Account session
versions, conditional refresh rotation and explicit replay outcomes. JWT format, browser transport,
permission projection and revocation are separate contracts.

The choice preserves AG's existing immediate single-session revocation. AD's current access checks
use Account status/session_version and do not bind access JWTs to one session through `sid`. AG therefore
adds mandatory `sid` binding and authoritative session validation; copying the architecture does not
silently replace immediate revocation with an access-token expiry window.

This is an accepted target architecture, not evidence that JWT issuance or refresh routes exist today.
OAuth/OIDC provider issuance, Organizations, delegated permissions and grant audit remain separate
implementation slices. [The matrix](IMPLEMENTATION-MATRIX.md) records those boundaries.

## Credential Contracts

| Credential | Consumer | Required context | Durable authority |
| --- | --- | --- | --- |
| First-party access JWT | AG Account and human management APIs | `iss`, `aud`, `sub`, `sid`, `jti`, `iat`, `exp`, token type, session version | Active Account and live owned browser session |
| First-party refresh JWT | AG refresh endpoint only | Same identity/session/time binding, distinct token type and refresh audience, fresh JTI | Current refresh generation/JTI in that browser session |
| Downstream OAuth access token | Registered resource audience | RFC 0003 claims, scopes and tenant grant | Registered client, Membership and resource authorization |
| OIDC ID token | Registered relying party | OIDC identity assertions | OIDC authorization and subject contract |

First-party `sub` identifies the internal Account UUID within this instance. It is not the public or
pairwise OIDC subject; downstream subjects retain RFC 0002's separate mapping and privacy guarantees.
The Account and management APIs validate the exact first-party audience; any finer audience split
must be explicit. Possessing that audience token grants no system permission by itself. Refresh has
its own validation policy and cannot authenticate an ordinary API request.

Issuer is explicitly configured and stable for the instance, independent of editable public site URLs.
Neither Host/Origin/forwarded headers nor selecting another primary mail site changes `iss`. JWT
validation pins issuer, audience, token type, allowed algorithms and trusted keys; it requires bounded
claims and token size, expiry/issued-at checks and explicit clock skew. Unknown keys, malformed tokens,
future timestamps and access/refresh/ID-token substitution fail closed. Claims are verified before
they are used to look up Account/session state.

Session signing material is separate from the AEAD deployment `master_key`. AG owns its persistence,
key IDs, rotation/retirement and backup contract; AF accepts explicit key and validation policy inputs.
No key or algorithm is selected solely from an untrusted JWT header. Internal session keys do not
automatically become downstream JWKS keys. RFC 0003's asymmetric signing requirement for downstream
tokens remains unchanged. The final internal algorithm/key representation is selected with AF #42's
implementation and must have algorithm-confusion and rotation fixtures before adoption.

Access lifetime is short; the initial implementation should use five minutes unless a reviewed policy
selects another bounded duration. Refresh lifetime is the session's configured absolute expiry, not
an unbounded sliding extension; the current 24-hour default is preserved until an explicit policy
changes it. Rotation does not resurrect expired or revoked sessions.

## Authentication And Permission Projection

```text
access JWT validation
  -> exact instance / audience / type / time / signature
  -> Account(sub) active and session_version current
  -> browser_session(sid) active, unexpired and owned by Account
  -> explicit service operation
  -> current system permission, ownership or tenant policy
  -> current-password step-up and transaction recheck when required
```

Both Account and management authentication check the writer-authoritative session and Account.
A cached validation result cannot accept a revoked session while waiting for pub/sub invalidation.
Failure to establish current security state rejects the operation rather than falling back to claims.
JWT signature validation is proof of issuance, not proof that the session or permission is still active.

`GET /api/v1/account` continues to project current `system_permissions` for frontend controls. Exact
registered permissions remain authoritative backend checks; a hidden button is not enforcement.
JWTs do not embed the complete permission list, memberships or mutable profile. Removing a grant
must deny the next relevant operation even while the access JWT has not expired. Suspend/restore/delete
and settings update retain password proof, transaction-time Account/session/grant revalidation and
critical last-holder protection. JWTs do not turn setup operators into tenant administrators.

## Durable Session And Refresh Lifecycle

Account `session_version` is a security epoch distinct from its existing mutation `revision`.
Ordinary profile edits and successful token refresh do not advance the global epoch or revoke
unrelated sessions. Session records own `sid`, Account, version at creation, absolute expiry,
revocation state, current refresh JTI/generation, bounded predecessor/replay evidence and presentation
metadata. Full access/refresh token strings are not persisted in ordinary rows, caches or audit logs.

Login atomically creates the owned session and initial refresh generation after Account/password
checks. Tokens and Cookie headers are returned only after the transaction commits. Concurrent
password/status changes fence the login so no usable token escapes from a stale proof.

Refresh validates the refresh-specific JWT policy and current Account/session version, then claims
the presented current JTI/generation with a conditional write inside the product transaction. Success
commits the next generation and its audit facts before returning the new pair. Any replica may finish
the rotation; the DB condition is the authority, while a per-JTI process lock only helps identify a
same-process competitor. Lock ownership is bounded and reclaimable, not a global product singleton.

| Outcome | Response and effects |
| --- | --- |
| Current generation, successful commit | One new access/refresh pair; previous refresh cannot succeed again |
| Recognized stale retry/concurrent loser | Typed rejection; no replacement token, no second successful rotation |
| Confirmed refresh reuse | Reject; commit configured security revocation and audit before returning rejection |
| Expired/revoked session or version mismatch | Reject; no renewal or resurrection |
| DB/CAS/audit/commit failure | No success response or token disclosure; distinguish rollback from committed security effects |

AG initially follows AD's conservative confirmed-reuse response: advance the Account session version
and revoke its first-party sessions. This is global first-party compromise handling, not the separate
tenant-grant replay policy in RFC 0003. Stale classification requires explicit evidence; there is no
generic grace window in which an old refresh succeeds. AD's recent predecessor/client-fingerprint
classification is a reference requiring boundary tests, not a device identity proof: UA/IP are
spoofable, and matching them never grants authentication. Older generations and replay after restart
remain detectable rather than disappearing when the immediate predecessor changes.

Single-session revocation marks its `sid` inactive, immediately rejecting both token types without
invalidating unrelated sessions. All-session logout, password change/reset, confirmed email replacement,
suspension, deletion and confirmed reuse invalidate the appropriate sessions and advance the security
epoch atomically when global invalidation is required. Restoration requires new login; old versions
and JTIs never become valid again. Token-signing rotation is a separate key lifecycle operation.

## Browser Transport And Frontend Refresh

The same-origin hosted UI uses separate HttpOnly access and refresh Cookies, following AD. Production
Cookies are Secure, SameSite=Lax and host-only. Access uses `/`; refresh is limited to the authentication
route scope. A restricted refresh Path requires a `__Secure-` rather than `__Host-` prefix if prefixed
names are used; `__Host-` requires Path `/`. Clear operations exactly match the original names, paths
and security attributes. Multi-site hosts keep independent Cookies and logins.

Login, refresh and logout keep existing Origin/custom-header CSRF protection and no-store responses.
JWTs and long-lived refresh credentials never enter localStorage or JavaScript-readable Cookies.
Browser token delivery uses Set-Cookie rather than plaintext JSON. A future Bearer API profile must
declare accepted credential types and reject ambiguous Cookie/Bearer identities; it is not added
implicitly by selecting JWT. First-party refresh is not the OAuth `/token` endpoint.

The frontend coordinates refresh within a browser tab, retries an eligible failed request at most
once, and does not refresh on permission denial or invalid password proof. Cross-tab and cross-replica
refresh races have one committed winner. Losing callers cannot overwrite new Cookies with stale
credentials or enter refresh loops; typed stale/reuse/session-expired outcomes drive bounded recovery
or sign-in. Issuing a token pair is not a substitute for the existing settings/resource revision fences.

## AF And AG Responsibilities

AF #42 should supply product-neutral typed token verification/issuance, refresh rotation outcomes and
storage-port contracts using a mature JWT library, with injected clock/ID sources, cancellation,
resource limits and independent tests. An auth crate is proposed there; its name/API is not available
today. `crypto`, `utils` and upstream `external_auth` do not own a product session subsystem.

AG owns Account UUID/status, schema/repositories, session epochs, permissions, tenant rules, signing
policy/keys, transactions, HTTP/Cookies/CSRF, API error mapping, UI and audit. AF returns typed security
facts; AG commits required revoke/version/audit facts together. `system_permission_granted` and future
grant/revoke events belong in durable audit logs with actor/target/permission/outcome, not token claims
or ordinary logging. Audit attribution improvements remain the separately tracked follow-up.

Post-commit cache/notification failure cannot be reported as if the security transaction rolled back.
The authoritative revoke decision remains effective even when cache invalidation fails. AG's immediate
revocation cannot depend on the unfinished configuration retry mechanism in AF #38.

## Adoption And Acceptance

AG is unreleased: replace opaque session transport and its callers in the same implementation,
without dual validators, token-format guessing or legacy-cookie aliases. Reuse the owned browser-session
aggregate rather than building parallel JWT session storage. Keep schema changes in the approved
initial migration while this development policy applies; do not reset user databases implicitly.
Published AD has its own compatibility obligations and must not inherit AG's no-compatibility rule.

Completion requires AF mechanism acceptance and AG schema/service/API/frontend/generated-contract
integration, followed by real-router and browser tests:

1. Strict signature/algorithm/key/issuer/audience/type/claims/time/size validation and token redaction.
2. Single/global revocation, status/credential changes and grant removal deny unexpired access JWTs;
   restoration, stale caches, different hosts and other Accounts cannot bypass those boundaries.
3. Same-process, cross-tab and multi-replica refresh contention has one committed winner; stale/reuse,
   predecessor-window limits, forged fingerprints, old generations and restart replay are covered.
4. Transaction/audit/commit failure exposes no usable tokens; confirmed-reuse revocation is not
   rolled back by the authentication error; post-commit cache/notification failures are distinguished.
5. Host-only/path-correct Cookie issuance and clearing, CSRF, no browser secret storage, bounded UI
   retries, session expiry and refresh failure recovery work in real browsers.
6. Account/system-permission regression suites pass on SQLite and necessary PostgreSQL/MySQL paths;
   OpenAPI and generated frontend types match the final routes, errors and responses.

Current opaque-session tests establish today's lifecycle, not these JWT/refresh acceptance gates.

## References

- [AD issuance/validation](https://github.com/AsterCommunity/AsterDrive/blob/cb4c0874eeae6a2d37808a32883962d3499f986d/src/services/auth/local/tokens/mod.rs)
- [AD rotation/reuse](https://github.com/AsterCommunity/AsterDrive/blob/cb4c0874eeae6a2d37808a32883962d3499f986d/src/services/auth/local/tokens/refresh.rs)
- [AD Cookie transport](https://github.com/AsterCommunity/AsterDrive/blob/cb4c0874eeae6a2d37808a32883962d3499f986d/src/api/routes/auth/cookies.rs)
- [AF #42 mechanism and integration acceptance](https://github.com/AsterCommunity/AsterForge/issues/42)
