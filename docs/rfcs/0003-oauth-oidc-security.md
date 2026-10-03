# RFC 0003: OAuth 2.1 and OpenID Connect Security Contract

- Status: Proposed
- Date: 2026-09-13
- Owners: AsterGate maintainers
- Depends on: RFC 0001, RFC 0002

## Summary

AsterGate's downstream OIDC profile uses Authorization Code with PKCE. OAuth extensions are
advertised only as complete, conformance-tested vertical slices. Protocol endpoints use
standard wire behavior and are isolated from the product REST envelope.

OIDC OP and OAuth AS are downstream adapters over one client registration, grant, authorization
code, token/key, session, refresh-family, and revocation core. OIDC-specific claims, UserInfo,
discovery, and logout do not introduce an independent issuance path (RFC 0006).

This RFC states minimum security behavior. Competitor compatibility cannot weaken these rules.

## Normative baseline

Implementation must track the current published specifications and applicable errata, including:

- [OpenID Connect Core 1.0](https://openid.net/specs/openid-connect-core-1_0.html)
- [OpenID Connect Discovery 1.0](https://openid.net/specs/openid-connect-discovery-1_0.html)
- [OAuth 2.0 Authorization Framework, RFC 6749](https://www.rfc-editor.org/rfc/rfc6749)
- [OAuth 2.0 Security Best Current Practice, RFC 9700](https://www.rfc-editor.org/rfc/rfc9700)
- [PKCE, RFC 7636](https://www.rfc-editor.org/rfc/rfc7636)
- [Authorization Server Metadata, RFC 8414](https://www.rfc-editor.org/rfc/rfc8414)
- [Token Revocation, RFC 7009](https://www.rfc-editor.org/rfc/rfc7009)
- [Token Introspection, RFC 7662](https://www.rfc-editor.org/rfc/rfc7662)
- [Resource Indicators, RFC 8707](https://www.rfc-editor.org/rfc/rfc8707)
- [OAuth 2.0 Device Authorization Grant, RFC 8628](https://www.rfc-editor.org/rfc/rfc8628)
- [OAuth 2.0 for Native Apps, RFC 8252](https://www.rfc-editor.org/rfc/rfc8252)
- [JWT, RFC 7519](https://www.rfc-editor.org/rfc/rfc7519) and the applicable JOSE specifications

OAuth 2.1 is treated as the target profile: deprecated or insecure grants are excluded even where
legacy OAuth 2.0 text describes them.

## Issuer and required endpoints

Every protocol endpoint is relative to the logical instance's one canonical issuer. The instance
owns the issuer/signing domain, not an upstream IdP or an organization. The deployment may use
its origin as issuer; a custom domain must be configured as one exact, stable `issuer`. Request
headers do not manufacture an issuer dynamically, and organizations do not get separate issuers.
Independent identity domains use independent instances with separate key/state ownership;
replicas of one instance share the same issuer. RFC 0007 defines that boundary and replaces the
singleton Realm entity terminology.

The conformance-capable provider exposes the following issuer-relative surfaces (the final paths
are fixed before issuing durable production identities):

```text
GET  {OIDC discovery location for issuer}
GET  {OAuth authorization-server metadata location for issuer}
GET  {issuer}/jwks.json
GET  {issuer}/authorize
POST {issuer}/token
GET  {issuer}/userinfo
POST {issuer}/revoke
POST {issuer}/introspect
GET|POST {issuer}/logout
```

Later slices may add device authorization, pushed authorization requests, back-channel logout,
token exchange, dynamic client registration, and CIBA. They are not advertised in discovery until
fully implemented and tested.

Downstream connector capability discovery also advertises only implemented profiles. Upstream
OIDC/OAuth is an RP-side identity input; its callback/token exchange is not an OP issuance
endpoint, and its issuer never replaces the one AsterGate issuer.

## Authorization request

The authorization endpoint:

- accepts registered clients only;
- resolves the client-owned organization before authentication and rejects cross-organization
  Membership, group, connector, or grant references;
- validates response type, client, exact redirect URI, requested scopes, resource indicators,
  prompt, max age, and OIDC nonce requirements;
- requires PKCE for public clients and defaults to requiring it for all clients;
- accepts only `S256`; `plain` is not supported;
- binds the interaction to a browser secret in an HttpOnly, Secure, SameSite cookie or an
  equivalent proven mechanism;
- preserves validated request data server-side under an opaque, short-lived interaction handle;
- never accepts arbitrary post-login redirect paths outside registered client URIs;
- requires an active Membership for the authenticated global Account in the client's organization;
- checks the application's group admission policy before consent or code issuance; a denied
  Membership receives no code even when Account authentication succeeded;
- presents consent only after authentication, Membership, and admission evaluation.

`state` is returned to the client unchanged but is not treated as server-side CSRF protection. The
server's interaction and browser binding protect the hosted flow. OIDC `nonce` is copied into the
ID token and validated by the client.

## Authorization codes

Authorization codes are high-entropy, opaque, one-time credentials. Persistence stores only a
hash and the bound facts:

- AsterGate issuer, organization, application/client, global Account, target Membership, browser
  session, and tenant grant;
- exact redirect URI;
- PKCE challenge and method;
- approved scopes and resources;
- OIDC nonce and authentication context;
- creation, expiry, and consumed timestamp.

The default lifetime is at most five minutes and should be shorter after usability testing. Code
consumption is one conditional database update in the same transaction that creates the resulting
token family or grant state. A mismatch does not consume some other valid code, and a concurrent
second exchange loses deterministically.

OAuth and OIDC authorization requests consume this same code repository and token endpoint. A
connector must not add its own code store or issuance transaction.

## Token endpoint and client authentication

The token endpoint accepts `application/x-www-form-urlencoded`. Supported initial methods are:

- public client with `client_id` and PKCE;
- confidential client using `client_secret_basic`;
- confidential client using `private_key_jwt` after its key-validation slice exists.

`client_secret_post` may be supported for compatibility but is not the preferred method. Client
authentication behavior is configured explicitly; the server never retries the same one-time code
with a guessed alternative method.

Client credentials grant is limited to confidential machine applications and cannot impersonate a
user. Resource-owner password credentials and implicit grants are never supported.

## Access tokens

JWT access tokens are issued for an explicit resource audience. They contain only the authorized
subject/client, audience, scope, time, issuer, token ID, and explicitly configured organization or
custom claim context.

For an organization-owned client, tenant context comes from the registered client and the
authenticated Account's active Membership, never a caller-selected claim. A `groups` claim is a
per-client allowlisted projection of that Membership's active local groups when the relying party
needs group compatibility; it does not bypass admission or resource-server authorization. A token
bound to one organization/audience cannot be relabeled or reused for another. Account-area
organization switching starts a new authorization for the target client/audience and issues new
tokens after its Membership and policy checks.

Opaque tokens may be used for AsterGate-owned Account/UserInfo access or deployments requiring
central introspection. Token format is discoverable by contract, not guessed by clients.

Defaults:

- asymmetric signing only;
- short access-token lifetime, initially 5 to 15 minutes depending on audience risk;
- exact `iss` and `aud` validation by resource servers;
- algorithm allowlist with no acceptance of token-selected algorithms outside instance policy;
- no bearer token in query parameters;
- no secret or mutable full profile embedded in tokens.

## ID tokens and UserInfo

ID tokens follow OIDC requirements for `iss`, `sub`, `aud`, `exp`, `iat`, `auth_time`, `nonce`, and
multiple-audience handling. Optional authentication method/reference claims are emitted only from
verified interaction state.

UserInfo requires an access token intended for that endpoint. Claims are filtered by granted OIDC
scopes and claim policy. Upstream raw claims are not forwarded automatically.

## Refresh-token families

Refresh tokens are opaque random values stored only as hashes. Each issued token belongs to a
family and points to its predecessor or generation. Rotation is atomic:

1. lock or conditionally consume the presented active token;
2. re-evaluate Account, browser session, target Membership, client, group admission, tenant grant,
   issuer, and organization policy;
3. issue the next token and access token in the same transaction;
4. return plaintext only after commit.

Reuse of an already rotated token revokes its tenant-bound family/grant, records a security audit
event, and does not issue replacement credentials. Global browser-session revocation is a separate
compromise policy decision, not an accidental effect on the Account's other organizations.
Concurrent legitimate refresh attempts yield one winner.

## Sessions, logout, and revocation

A browser session represents authenticated global Account state; a grant represents authorization
for one client, organization, and Membership; a refresh family represents renewable credentials
for that grant. They are related but not interchangeable.

- Application-local logout may revoke only its tenant grant/family. AsterGate global logout
  terminates the Account browser session across organizations, without deleting Memberships.
- RP-initiated OP logout validates ID-token hint and registered post-logout URI, then terminates
  the AsterGate browser session according to the declared OIDC logout policy; it is not silently
  treated as application-local grant revocation.
- Revocation invalidates the supplied token or family according to RFC 7009 without exposing token
  existence to unauthorized callers.
- Introspection returns active only after checking token, client/audience, Account, Membership,
  organization, session, grant, expiry, and revocation state.
- Back-channel logout is a separate later feature with delivery and retry evidence.

## Signing keys

Signing-key lifecycle states are `staged`, `active`, `retiring`, and `retired`:

```text
generate staged
  -> publish public JWK
  -> activate for new signatures
  -> retain previous public JWK through maximum verification window
  -> retire and eventually destroy private material
```

Private keys are encrypted at rest or held by an explicit KMS/HSM provider. They never enter logs,
API responses, backups without encryption, or ordinary configuration serialization. Each JWT has a
`kid`; verifiers reject unknown keys and algorithms. Emergency compromise rotation is distinct
from planned rotation and can revoke affected sessions/grants.

`RS256` is the required initial ID-token and JWT access-token signing algorithm for broad OIDC
client compatibility. `none` and symmetric ID-token signing are not supported. Additional
asymmetric algorithms such as `ES256` are advertised only after their independent conformance,
rotation, and cross-client tests pass. Algorithm selection is server policy and is never accepted
solely from an untrusted JWT header.

## Browser and REST security

- Hosted browser session cookies are HttpOnly, Secure in non-loopback deployments, appropriately
  SameSite, host-scoped where possible, and rotated after authentication level changes.
- State-changing Experience, Account, and Management APIs enforce origin/CSRF policy when using
  cookies.
- CORS allowlists are application/surface specific and never derived from arbitrary redirect URIs.
- Sensitive responses use `Cache-Control: no-store`; protocol responses set required anti-sniffing
  and framing headers.
- Login, recovery, factor, token, introspection, and management endpoints have separate rate-limit
  keys and privacy-preserving failure responses.
- Forwarded host/scheme/address data is accepted only from configured trusted proxies. Issuer and
  redirect generation does not trust arbitrary request headers.

## Credential and account security

- Passwords use a memory-hard password hash with versioned parameters and opportunistic rehash.
- Enumeration-sensitive registration, reset, and verification requests use stable public outcomes
  and bounded timing behavior.
- MFA and recovery transitions have TTL, attempt budgets, single consumption, and explicit replay
  tests.
- External identity linking requires authenticated proof of Account control and verified upstream
  subject; matching email alone never links Accounts or grants organization Membership.
- Step-up authentication is required for security settings, impersonation, key management, and
  other high-risk actions.

## Upstream federation security

The AsterDrive-derived upstream flow keeps these properties:

- state and browser-binding secrets are independent and stored only as hashes where lookup permits;
- OIDC uses discovery, PKCE S256, nonce, ID-token signature, audience, issuer, and time validation;
- generic OAuth uses one configured client authentication method and does not pretend UserInfo is
  an ID token;
- identity is namespace plus subject, never email alone;
- connector-specific verified-email semantics are explicit;
- callback errors return a stable public result while detailed diagnostics remain redacted.

SSRF controls are added before accepting administrator-provided discovery, JWKS, metadata, token,
or UserInfo URLs: scheme/port policy, DNS/IP range checks, redirect limits, response size/time
limits, and revalidation after redirects and DNS resolution.

## Dependency policy

AsterGate will not implement signature primitives, JWT parsing, password hashing, WebAuthn, XML
signature, or HTTP TLS itself. Before protocol implementation, a time-boxed dependency spike must:

- identify actively maintained Rust crates for JOSE and relevant protocol primitives;
- review algorithm support, parsing strictness, key rotation fit, security advisories, licensing,
  async/runtime compatibility, and fuzz/property-test suitability;
- demonstrate authorization-code and ID-token golden vectors without hiding persistence or product
  policy inside an opaque framework;
- record rejected candidates and reasons.

The `openidconnect` crate used by `aster_forge_external_auth` is an RP/client implementation. It
must not be mistaken for AsterGate's OpenID Provider implementation.

## Verification gates

Before calling the OIDC slice production-ready, evidence must include:

- unit tests for validation and all state transitions;
- concurrent authorization-code and refresh-token consumption tests;
- malformed request, redirect URI, PKCE, nonce, issuer, audience, time, and algorithm tests;
- JWKS/key rotation and cache rollover tests;
- SQLite plus supported PostgreSQL/MySQL repository tests;
- integration tests using an independent OIDC client;
- official OpenID conformance suite results for the supported profile;
- fuzzing or property testing for security-critical parsers owned by this repository;
- log/audit snapshots proving secret redaction.

Compile success or a happy-path browser login is not protocol conformance.
