# RFC 0002: Identity, Tenancy, and Authorization Model

- Status: Proposed
- Date: 2026-09-13
- Owners: AsterGate maintainers
- Depends on: RFC 0001

## Summary

AsterGate has one canonical OIDC issuer and signing domain per logical instance. A global `account`
owns credentials, external identities, profile, and browser sessions. An `organization` is a
tenant; a `membership` is an Account's identity and status inside that organization. One Account
may hold memberships in many organizations. An organization owns applications and user groups;
group membership binds tenant Memberships, not global Accounts, to application admission and
organization permissions.

These boundaries are not interchangeable. The instance issuer identifies AsterGate-issued tokens,
never an upstream IdP. Account answers who authenticated; Membership answers in which tenant they are
active; Group answers which applications and organization permissions that Membership receives.
An email address, upstream claim, or OAuth scope cannot substitute for any of them.

## Core concepts

### Instance identity domain

A logical AsterGate instance owns the canonical issuer URL, signing keys, subject policy, and
protocol defaults. All organizations in that instance share its issuer. Account credentials,
external identities, and browser sessions are global within the instance, not across deployments.

Instance configuration owns issuer/subject policy, authentication/session/token defaults, and
global branding/locale defaults. Organization overrides are explicitly bounded. There is no
Realm product entity, Realm ID, selector, or CRUD; bootstrap, recovery, and retirement are
operational procedures. RFC 0007 replaces the earlier singleton Realm ID/lifecycle proposal.

There is no per-organization issuer routing. A separate identity domain uses an independent
instance; changing an existing issuer requires an explicit migration contract. Untrusted request
headers and email domains cannot manufacture either the issuer or the active organization.

### Organization

An organization owns tenant policy, Memberships, user groups, applications, API resources, roles,
connectors, grants, and organization-scoped authorization state. It does not own or clone an
Account's credentials, external identities, or browser session. Each `(organization_id,
account_id)` pair has at most one Membership with its own status and tenant profile. Account
authentication alone does not create or activate a Membership.

`client_id` resolves the application and its owning organization before authorization. An Account
must hold an active Membership there and pass application admission. Suspending an organization
blocks its clients and Memberships without globally disabling their Accounts or other memberships.
Deletion is a retention and revocation workflow, not a blind database cascade.

Upstream connector instances configured by this organization authenticate global Accounts but
cannot create Membership or Group access as a side effect of identity matching. Downstream
Application protocol connectors use the same Membership and Group admission decision regardless
of OIDC/OAuth or another supported wire format (RFC 0006).

The Account self-service area may list the authenticated Account's Memberships from its global
browser session. Selecting another organization changes the UI context only; access to that
organization's application or API requires a fresh authorization for its registered client and
audience, yielding new tenant-bound credentials after Membership and admission checks. Existing
tokens never change organization.

### User group

A user group is an organization-owned set of Memberships, similar to an LDAP group in its
application-access use case. A Membership may join multiple groups in its organization. The
`group_memberships` join records have explicit lifecycle and audit; cross-organization binding is
invalid. Groups may be local or synchronized from a trusted directory, but an upstream group name
is not trusted until a connector maps it to a local group under an explicit policy.

### Application and API resource

An Application is an Org-owned integration. Its OIDC/OAuth `client_id` uniquely resolves that
organization under the sole AsterGate issuer; a future SAML2 binding has its own SP identifier
mapped to the same Application and Org. OAuth clients have a type, redirect/logout URIs,
authentication method, allowed grants, consent policy, and access-control policy. Clients do not
switch tenant by supplying an organization parameter.

An Application may expose more than one downstream protocol under an explicit registration and
compatibility contract. OIDC OP and OAuth AS reuse its same client registration, grants, codes,
and tokens; a future SAML2 SP binding adds protocol-specific metadata without duplicating
the Account, Membership, or Group facts (RFC 0006).

An API resource is a protected resource identified by a stable URI audience. Scopes belong to an
API resource or to the OIDC identity surface. Applications request scopes; roles grant permissions;
resource servers still enforce the resulting audience and scope claims.

## Identity model

```text
AsterGate logical instance (one issuer and signing domain)
  +-- Account -> identifiers, credentials, external identities, browser sessions
  +-- Organization A (tenant)
  |    +-- Membership -> Account
  |    +-- user_groups -> group_memberships -> Memberships
  |    +-- applications -> application_access_policies -> user_groups
  |    +-- API resources, scopes, roles, group role assignments
  |    +-- upstream connectors and tenant grants
  +-- Organization B -> a different Membership may point to the same Account
```

### Account and membership

The Account is the stable global authentication aggregate. Its status and profile are independent
of organization Membership status; credentials, external identities, and browser sessions are
separate global records. Expected Account statuses include `active`, `suspended`, `locked`, and
`pending_deletion`. A Membership has its own `active`, `suspended`, and `left` lifecycle and may
carry a tenant display profile. Leaving one organization does not delete the Account or other
Memberships. Irreversible Account deletion is a separate retention workflow across all tenants.

Internal Account and Membership database IDs never serve directly as public OIDC subjects.
First-party session JWTs may identify the internal Account UUID as their private `sub`; this does
not alter public/pairwise OIDC subjects. RFC 0009 defines access/refresh JWTs bound to a live `sid`
and Account `session_version`, separate from ordinary mutation revisions and tenant grants.

### Identifier

Email, phone, and username are typed identifiers with normalized and display values. Verification
belongs to the identifier record, not to the Account as a single global Boolean.

Required uniqueness for active verified login-capable identifiers is `(kind, normalized_value)`
within AsterGate's global Account directory; unverified and retired values have explicit
collision/recovery rules. Normalization and verification are kind-specific. A matching email
never links an external identity or grants organization Membership. Changing an email verifies
the new identifier before retiring the old one; it does not rewrite upstream identity subjects.

### Credential and factor

Credentials are purpose-specific records:

- password hash and password policy metadata;
- WebAuthn credential public data and sign counter;
- TOTP secret envelope and recovery-code set;
- future device or platform factors.

Secrets are encrypted or one-way hashed according to their use. The schema must not place all
credential payloads in a single unvalidated JSON column. Factor enrollment and deletion require
recent authentication and must prevent accidental removal of the last usable method unless the
global Account policy explicitly allows externally managed accounts.

### Upstream identity

An external identity is keyed by:

```text
(identity_namespace, subject)
```

`identity_namespace` derives from a verified upstream issuer or a stable provider namespace for
non-OIDC integrations. It remains the same across organization connectors to the same trust
domain; an organization ID or connector-row ID must not fragment one upstream identity into
multiple Accounts. Email and profile fields are snapshots, not identity keys. Provider-reported
`email_verified` is trusted only when the connector contract establishes its reliability.

The same external identity cannot be linked to two Accounts globally. Linking to an existing
Account requires one of:

- an already authenticated Account performing a protected link flow;
- successful proof of Account control and an explicit link decision.

Matching email or provider profile data never automatically links Accounts or enrolls one into
an organization. Membership creation is a separate invitation, provisioning, or explicitly
authorized JIT decision, audited independently of global Account authentication.

## Subject identifiers

AsterGate supports OIDC public and pairwise subjects:

- **Pairwise** is the default. A stable opaque `sub` is derived or stored per Account and sector
  identifier so unrelated clients cannot correlate users.
- **Public** may be enabled for explicitly trusted first-party application groups that need one
  common subject.

Subject generation is based on the global Account and client sector, not on Membership or a
mutable organization name. It uses a versioned server secret/key identifier or stored random
mapping and remains stable for the same client/sector across client-secret rotation and display-
name changes. Another organization's client may receive a different pairwise `sub` for the same
Account. Organization identity is a separate token context; switching organization or audience
requires a newly authorized token, never mutation of an existing token. Changing issuer or
subject mode after production requires an explicit migration RFC.

## Application model

Application types:

| Type | Client authentication | Primary grants |
| --- | --- | --- |
| traditional web | confidential | authorization code, refresh token |
| single-page app | public | authorization code + PKCE, refresh token policy |
| native | public | authorization code + PKCE, refresh token policy |
| machine-to-machine | confidential | client credentials |
| device | public | device authorization, refresh token policy |

Redirect URIs and post-logout redirect URIs are normalized and stored as child records. Production
authorization uses exact matching. Loopback native redirect ports and other narrowly defined RFC
exceptions receive explicit validators; a general wildcard is not the default.

Client secrets are versioned credentials stored as hashes. Creation is the only time plaintext is
returned. Rotation can overlap old and new credentials for a bounded interval, and revocation is
audited.

## Authorization model

### Definitions

- **Scope/permission:** a named action attached to an API resource or organization policy.
- **Role:** a named set of permissions in an organization.
- **Assignment:** a tenant user group or machine client receives a role in its organization.
- **Grant/consent:** authorization for a client to receive approved scopes for a subject.

Application admission and resource authorization are distinct:

```text
Account -> active Membership -> group membership -> application admission
Account -> active Membership -> group membership -> role -> permissions
```

The token service computes effective permissions from the target Membership's authoritative group
and role assignments when issuing or refreshing a token. Resource servers validate signature,
issuer, audience, expiry, organization context, and required scope. They retain domain checks
such as AsterDrive workspace membership and file ownership. Group admission is never a substitute
for those checks.

### Token projection

This section describes downstream audience/tenant projection. First-party Account/management JWTs
follow RFC 0009: identity/session binding plus live authorization, rather than a frozen full grant list.
The frontend reads current `system_permissions`; services recheck exact permissions and high-risk
password proof regardless of claims or button visibility.

Tokens expose the minimum context required by the audience:

- `iss`, `sub`, `aud`, `iat`, `exp`, and token identifier where appropriate;
- granted scope;
- client ID;
- organization ID as the target tenant context for organization-owned applications/resources;
- only application-approved group identifiers or names in a `groups` claim when a relying party
  needs them; raw directory groups and unrelated tenant groups are never projected;
- role names only when a stable consumer contract requires them; permissions are preferred for
  enforcement.

Profile, connector details, raw upstream claims, credential state, and full memberships are not
dumped into every token. Custom claims use a namespaced, allowlisted mapping with size limits and
reserved-claim protection.

### Application access policy

Each application has an explicit admission mode: `all_active_members` or `allowed_groups`. The
default is `allowed_groups` with no grants, which denies access until a group is bound. Evaluation
always requires an active organization, Account, Membership, and application; `allowed_groups`
also requires the target Membership in at least one bound active group. It runs before consent or
authorization-code issuance and again on refresh; a denied Account receives no code or new token.
Membership or admission-policy changes invalidate affected renewable grants. A previously issued
JWT remains valid until expiry unless the resource server introspects or receives revocation events.
This latency must be visible to operators.

The k8s-ops pattern uses centrally managed groups such as `developer`, `database-maintainer`, and
`cluster-admin` for application admission. These are examples, not reserved AsterGate roles.
AsterGate stores stable group IDs scoped to the organization; optional names in `groups` are a
deliberate per-client compatibility projection, not the source of truth. Both forbidden and
allowed login paths are part of acceptance.

## Organization lifecycle

Organizations own Memberships, groups, invitations, verified domains, and optional enterprise SSO
connector associations. Supported onboarding paths are:

- administrator invitation;
- Management API provisioning;
- upstream enterprise SSO JIT provisioning;
- verified-domain JIT provisioning under explicit instance and organization policy.

Domain matching never proves organization authorization by itself. The identifier must be
verified, the domain mapping must be unambiguous, and the JIT rule must explicitly authorize a
Membership and its resulting groups; matching email alone never does either. Existing Memberships
are not silently rewritten when JIT policy changes. Group deletion and sync remove application
admission before physical cleanup, and every change is audited.

## Proposed durable entities

The durable model reserves the following ownership boundaries. Exact columns and indexes are
defined with the relevant implementation slice. Instance configuration is a separate operational
contract (RFC 0007); any persistence for mutable global settings is defined with its implementation,
without a Realm entity.

```text
accounts
account_identifiers
password_credentials
webauthn_credentials
totp_factors
recovery_code_sets
account_external_identities
browser_sessions
organizations
organization_memberships
user_groups
group_memberships
applications
application_redirect_uris
application_credentials
application_access_policies
application_allowed_groups
api_resources
scopes
roles
role_permissions
group_role_assignments
organization_domains
upstream_connectors
oauth_grants
authorization_codes
refresh_token_families
refresh_tokens
```

Global Account/credential/external-identity/browser-session tables do not have `organization_id`.
Every tenant-owned table, including Memberships, group joins, applications, grants, codes, and
refresh families, includes `organization_id` even when it could be inferred. Uniqueness on
`(organization_id, account_id)` prevents duplicate Memberships; composite foreign keys and
repository validation prevent cross-organization joins and application/group references.
Application-visible keys use random or carefully generated stable identifiers rather than
sequential primary keys.

## Repository and transaction rules

- Global Account lookup is allowed only in authentication and self-service contexts. Tenant
  queries require an `OrganizationId` resolved from the client or authorized Account context,
  and an active Membership; an unscoped tenant `find_by_id` is not exposed to normal service code.
- Identifier creation, identity linking, invitation acceptance, Membership creation, group
  membership/admission, role assignment, authorization-code consumption, and refresh rotation use
  transactions plus unique/conditional writes.
- Unique conflicts are mapped to domain outcomes; preflight reads are not sufficient for
  concurrency correctness.
- Reader replicas are not used for read-after-write authentication and authorization decisions.
- Deleting a connector, application, group, role, scope, or organization checks references and
  applies a defined deactivate/revoke/retain policy before physical deletion.

## Administration boundaries

Management permissions are explicitly scoped, not a single `is_admin` bit. Instance setup creates
an operator Account authenticated through the same Account session machinery, with registered
`astergate.system.<resource>.<action>` permissions. It does not grant Organization membership or
tenant access. The current system-permission slice is defined in
[System authorization](../system-authorization.md); scoped role/delegation and Organization-owner
group provisioning remain future implementations. Operators manage
tenant provisioning and issuer keys but do not inherit tenant data access; organization owners
manage only their tenant. Explicit, audited elevation is required for cross-tenant support.
Administrative actions use the same role and audit model as automation.

High-risk operations require recent authentication or an equivalent machine credential policy:

- rotating signing or client keys;
- changing issuer/subject policy;
- disabling MFA requirements;
- impersonation;
- retiring the issuer or deleting organizations, Accounts, groups, connectors, or applications;
- modifying trusted domains or JIT Membership/group rules.

## Privacy and lifecycle

- Audit facts and operational telemetry store stable internal references and redacted summaries,
  not raw tokens or secrets.
- Upstream access/refresh token storage is opt-in per connector and consent, encrypted separately,
  and not required for sign-in.
- Account export, suspension, anonymization, and deletion have distinct global workflows;
  leaving or suspending a Membership affects only its organization.
- Retention periods are instance/organization policy but must not delete active signing verification
  material or audit evidence before their safety windows expire.

## AsterDrive boundary

AsterGate may issue AsterDrive audience tokens and shared scopes such as `openid`, `profile`, or
coarse application access. AsterDrive still owns teams, workspaces, policy groups, files, shares,
storage permissions, and their consistency. An AsterGate group grant must not be silently treated
as an AsterDrive workspace permission.

## Alternatives rejected

- **Organization as identity domain:** conflates business tenant ownership with issuer/signing
  isolation; independent domains use independent instances (RFC 0007).
- **Organization-owned duplicate Accounts:** splits credentials, sessions, and external identity
  for the same person and makes cross-organization participation depend on unsafe email matching.
- **Email-based tenant enrollment:** a matching identifier is not proof of Membership or group
  authorization.
- **LDAP group name as the identity key:** names can change and collide; stable tenant-scoped group
  IDs own assignments, while names are compatibility claims.
- **One identity row with nullable email/phone/provider columns:** cannot model multiple identities,
  verification, replacement, or provider-specific trust safely.
- **Email-derived `sub`:** leaks PII, changes over time, and correlates users across clients.
- **Only roles in tokens:** forces every resource server to share role semantics and makes
  permission evolution brittle.
