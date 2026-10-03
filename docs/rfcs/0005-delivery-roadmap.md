# RFC 0005: Final Product Contract and Delivery Gates

- Status: Proposed
- Date: 2026-09-13
- Owners: AsterGate maintainers
- Depends on: RFC 0001, RFC 0002, RFC 0003, RFC 0004

## Summary

AsterGate is a multi-tenant identity provider, not a sequence of temporary MVP architectures.
This RFC states the intended product capabilities and evidence required to call each one complete.
Implementation can be broken into dependency-ordered vertical slices without changing the durable
instance identity-domain, Account, Membership, group, application, protocol, or security boundaries.

A slice includes schema, repository, domain rules, transactions, wire behavior, frontend where
applicable, audit, observability, and tests. A screen, table, or token helper alone is not a
completed feature. Unimplemented capabilities are not advertised in discovery or the console.

## Final capability inventory

### Identity and experience

- Global Accounts with typed email, username, and phone identifiers, verification, suspension,
  import/export, deletion, and recovery workflows independent of tenant Membership lifecycle.
- Password, TOTP, recovery codes, email MFA, WebAuthn passkeys, and passwordless methods with
  explicit policy and recent-authentication requirements.
- Hosted sign-in, sign-up, consent, error, and account pages, with branding, locale, terms/privacy
  links, and equivalent custom-UI functionality through the Experience API.
- Self-service global profile, identifiers, credentials, factors, linked identities, browser
  sessions, organization Memberships, and authorized applications. Switching organization in the
  Account area requires fresh authorization and tokens for its client/audience.
- One AsterGate issuer/signing domain, tenant-scoped Membership and policy, and stable public or
  pairwise Account subjects that never use email or Membership IDs as identity.

### Multi-tenant organization and authorization

- Multiple organizations under the sole AsterGate issuer, each owning Memberships, groups,
  applications, connectors, roles, resources, and grants. One Account may join several
  organizations; a single-organization deployment uses the same schema and APIs.
- Organization user groups binding tenant Memberships with explicit lifecycle, local and
  trusted-directory synchronization, invitation and JIT provisioning, and auditable group changes.
- Application admission by active group membership. Each application explicitly selects
  `all_active_members` or `allowed_groups`; the latter denies access until at least one group is
  bound. Admission is checked before code issuance and on refresh.
- Organization roles, permissions, API resources and scopes, group-role assignments, machine
  principal assignments, and application-level access policy. Resource servers still enforce
  their own domain permissions.
- Verified domains and enterprise connector routing with ambiguity rejection. Domain or email
  matching alone never links Accounts, creates Membership, or confers group access.
- Scoped instance-operator and organization-admin permissions; no universal `is_admin` shortcut.

### Applications and protocol

- Traditional web, SPA, native, device, and confidential machine applications with exact redirect
  and logout URI registration, versioned hashed client credentials, consent, and grant management.
- OIDC Authorization Code + PKCE S256, discovery, authorization-server metadata, JWKS, ID token,
  UserInfo, refresh rotation/reuse detection, revocation, introspection, and logout.
- Audience-bound JWT or opaque access tokens, resource indicators, client credentials, device
  authorization, and explicit resource-server validation contracts.
- Token exchange, back-channel/front-channel logout, SAML application mode, dynamic clients,
  PAR/JAR/JARM, and CIBA only under their own protocol/security RFCs. They are not implied by
  partial route registration.
- Per-client, allowlisted `groups` claim projection for applications that need LDAP-like group
  compatibility. A claim is an output of the target Membership's groups and admission policy,
  never a source of identity or an automatic grant of application-domain privileges.

### Federation and enterprise

- Upstream LDAP, SAML2, OIDC and OAuth2 protocol families, provider-specific
  Google/Microsoft/social connectors, and enterprise federation with Org-scoped connector
  configuration and explicit global Account linking.
- Enterprise SAML where AsterGate acts as SP, with metadata, assertion, signature, audience,
  recipient, replay, clock-skew, encryption, and certificate rotation under a separate RFC.
- Verified-domain and enterprise JIT provisioning, directory/SCIM synchronization, and explicit
  mapping of trusted upstream groups onto authorized tenant Memberships and their local groups.
- Optional encrypted upstream delegated-token storage with separate consent and revocation.
  Upstream credentials are never returned as AsterGate-issued access tokens.
- Downstream OIDC/OAuth and future SAML2 Application connectors share Account authentication,
  Membership/Group admission and authorization decisions. OIDC OP and OAuth AS share the same
  client/grant/code/token/session/revocation core; protocol adapters do not become issuers of
  independent identity facts (RFC 0006).

### Developer and operator platform

- Management, Account, and Experience APIs; generated OpenAPI for product REST and normative
  metadata/conformance for OAuth/OIDC protocol surfaces.
- Rust/Aster client and resource-server integration guidance, signed webhooks with retry/replay
  semantics, and declarative allowlisted custom claims.
- Audit, low-cardinality metrics, health, cleanup, backup/restore, bootstrap, key rotation,
  disaster recovery, tenant suspension/deletion, and upgrade/rollback runbooks.
- Custom domains and issuer migration require an explicit compatibility procedure because issuer
  and subject identities are durable public contracts.
- Independent identity domains use independently configured instances with separate keys and
  identity state. Replica, isolation, federation, and recovery acceptance follows RFC 0007;
  there is no Realm product entity or speculative `realm_id` schema.

## Contract decisions before persistent identity

These are design inputs, not a temporary architecture:

1. Fix the sole canonical issuer URL, organization selection from registered client,
   supported client types, access-token formats, and initial signing algorithms.
2. Select and audit Rust JOSE/OAuth dependencies. Execute golden vectors for signing, PKCE,
   authorization-code hashing, and redirect URI validation; record rejected candidates.
3. Threat-model cross-organization references for one Account, group admission, token
   theft/replay, Account linking, Membership enrollment, SSRF, key compromise, operator abuse,
   and directory-group synchronization.
4. Define bootstrap and break-glass ownership, trusted proxy handling, database concurrency and
   recovery matrix, token lifetime, revocation latency, and compatibility policy.

No production issuer or durable subject is issued until those decisions are explicit.

## Delivery sequence

The sequence follows dependencies; it does not permit a disposable single-tenant implementation.

1. Canonical issuer and organization bootstrap, global Account/identifier/credential, tenant
   Membership/user group, and operator/owner model.
2. Signing-key store, JWKS, issuer metadata, rotation, and application/client registration.
3. Application group admission, redirect URI and consent policy, hosted interaction, and local
   sign-in. Test both denied and allowed group paths, plus the same Account's separate Memberships.
4. Authorization Code + PKCE, ID/access tokens, UserInfo, session/grant/refresh rotation,
   revocation, introspection, and logout.
5. Verification/recovery, Account/Management APIs, audit/metrics/cleanup, and AsterDrive client
   integration with explicit account linking and rollback.
6. Strong authentication, upstream federation (including LDAP/SAML2 family ports), organization
   JIT/group sync, API authorization, machine clients, downstream SAML2 and ecosystem extensions
   under their complete contracts.

Every item must preserve the final tenant and identity model. Internal compile checkpoints do not
count as a production-ready identity feature. The OIDC provider is not advertised as complete
until the independent-client and OpenID conformance gates pass.

## Cross-cutting acceptance

| Area | Required evidence |
| --- | --- |
| Domain | Allowed/forbidden transitions, expiry, attempt limits, terminal replay |
| Account, tenant, and group | Global external-subject uniqueness, no email-based Account link or Org enrollment, same Account in multiple Orgs, cross-tenant reference denial, group admission allow/deny, Membership removal |
| Persistence | Unique constraints, atomic consume/rotate, rollback, cleanup, concurrent one-winner behavior |
| Protocol | Golden vectors, malformed input, normative error wire format, independent client |
| Security | Threat-model cases, redaction, SSRF, CSRF/origin, rate limits, key compromise/rotation |
| Databases | SQLite plus declared PostgreSQL/MySQL matrix |
| API contract | OpenAPI generation and frontend SDK drift for product REST APIs |
| Frontend | Focused component tests and browser E2E for user-visible flows |
| Operations | Readiness, metrics, audit, backup/restore, migration and rollback |
| Conformance | Official suite for each advertised OAuth/OIDC or SAML2 profile where available; independent LDAP interoperability fixtures |
| Connectors | Descriptor/registry/configuration consistency, family-specific trust and wire tests, same core across upstream/downstream paths, new-provider and new-protocol compatibility gates |

The AsterDrive test deployment must prove issuer, signature, audience, nonce, PKCE, refresh,
revocation, logout, client-based tenant selection, Membership denial, group admission and
rejection, and product-local authorization. A successful login alone is not acceptance.

## Suggested implementation layout

The exact module tree may evolve, but ownership should remain recognizable:

```text
src/
  api/routes/{protocol,experience,account,management}/
  domain/{identity,organization,interaction,oauth,authorization}/
  services/{identity,organization,interaction,oauth,session,keys}/
  connectors/{catalog,upstream,downstream}/
  db/{entities,repository}/
  protocol/{oauth,oidc,saml2,ldap}/
crates/aster_gate_migration/src/
frontend-panel/src/pages/{sign-in,account,admin}/
```

Protocol serialization may become a dedicated internal crate if it materially reduces coupling;
splitting crates only to mirror directories is not a goal.

## Documentation with implementation

- Administrator deployment, tenant bootstrap, group admission, and application integration guides.
- Resource-server validation, account recovery, and break-glass runbooks.
- Signing/client key rotation and compromise, backup/restore, upgrade/rollback, and issuer
  migration runbooks.
- Supported protocol/profile/database matrix and security disclosure process.

## Separate decisions

Cloud provisioning, billing, quotas, marketplace, KMS/HSM provider integrations, user
impersonation, executable authentication actions, and additional SDK language order need their
own product or security contract. They do not change the tenant/group ownership decided here.
