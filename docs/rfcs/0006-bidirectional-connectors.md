# RFC 0006: Bidirectional Connector Architecture

- Status: Proposed
- Date: 2026-09-30
- Owners: AsterGate maintainers
- Depends on: RFC 0001, RFC 0002, RFC 0003, RFC 0004, RFC 0005

## Summary

AsterGate has two connector directions. An **upstream connector** authenticates an Account against
an external identity source and returns a normalized, verified external identity. A **downstream
connector** presents AsterGate as an identity/authorization provider to an Organization-owned
Application. Both directions share descriptor, registry, configuration-validation, secret-handling,
and capability-discovery conventions. They do not share a mandatory protocol flow trait: LDAP bind,
SAML2 assertions, OAuth callbacks, and downstream token endpoints have different wire and trust
contracts.

Every connector is an adapter to the same AsterGate product services. Global Account and external
identity, Org Membership and Group, Application admission, authorization, grants, sessions, keys,
and revocation have one owner. An adapter cannot create a parallel user store, decide tenant access
from raw upstream claims, or own independent issuance state and authorization policy.

## System boundary

```text
External IdP / directory                       Application / resource server
LDAP | SAML2 | OIDC | OAuth                    OIDC/OAuth | future SAML2
           |                                               ^
           v                                               |
  Upstream protocol adapters                     Downstream protocol adapters
           |                                               ^
           v                                               |
 normalized external identity                   authorized protocol result
           |                                               ^
           +----> Account authentication / session --------+
                        |              ^
                        v              |
                Org Membership -> Group admission
                        |              |
                        +-> authorization / grant / key / revocation core
```

The diagram is a trust-boundary view, not a separate set of services or databases. Each logical
AsterGate instance has one canonical OIDC issuer/signing domain (RFC 0007). An upstream OIDC issuer
is an input identity namespace; it never becomes AsterGate's downstream issuer. Application
`client_id` selects its Org before authorization. Account authentication does not imply Org
Membership or Group admission.
An independent AsterGate instance may be an explicitly trusted upstream provider; its Accounts,
sessions, and Org memberships do not become shared local state through that Connector.

## Shared connector contract

The shared catalog identifies `(direction, protocol_family, connector_kind)` with stable,
versioned wire identifiers. A descriptor declares only applicable capabilities, such as
discovery/metadata, browser redirect, credential verification, assertion validation, signing,
logout, or connection test. It also declares configuration/credential schema versions, supported
actions and profiles, administrative labels, and any supported UI fields. Absent capability means
unsupported, not an invitation to try a different protocol path.

Registration validates unique identifiers, descriptor/runtime consistency, supported protocol
profile, configuration schema, and feature availability at startup. Unknown or disabled kinds fail
closed. Product-owned saved configurations refer to the stable kind and schema version; credentials
are encrypted and redacted, and rotation has a bounded overlap or explicit cutover. Configuration
testing is side-effect-free with respect to Account, Membership, and grant state. Runtime lookup
uses an injected registry, not hidden global mutable state.

The catalog and administrative API may share list, descriptor, validation, secret-status,
draft/saved test, and localization mechanics. Storage of Org-owned upstream connector instances,
Application configuration, credential material, audit, and management permissions stays in
AsterGate. Downstream OIDC and OAuth must share the same application/client registration; no
duplicate client rows are created merely to fit a generic connector table. No descriptor bypasses
server-side validation or grants an operator permission to execute arbitrary code.

### Protocol-family interfaces

Each family owns only the methods its wire protocol needs:

| Direction/family | Adapter operations | Output into the product core |
| --- | --- | --- |
| Upstream OIDC/OAuth | start redirect, validate bound callback, exchange code, verify identity/profile | normalized namespace + subject and assurance evidence |
| Upstream SAML2 | load trusted metadata, initiate/receive assertion, verify signature/conditions/replay | normalized namespace + subject and assurance evidence |
| Upstream LDAP | establish protected connection, bind/search with bounded queries, verify credential/directory identity | normalized namespace + subject and assurance evidence |
| Downstream OIDC/OAuth | validate client/request, render protocol endpoints and responses | shared authorization decision, code/token/grant/session lifecycle |
| Downstream SAML2 | validate SP/ACS/request, construct and sign assertion, handle logout contract | shared Account, Membership, Group and authorization decision |

These are interface responsibilities, not one Rust trait with every method optional. Common
descriptor/configuration interfaces compose with typed protocol-family ports. LDAP and SAML2 do
not implement OAuth `start_authorization` or `exchange_callback` merely to enter the registry.
Likewise, downstream SAML2 assertion serialization does not pretend to create an OAuth code or
refresh family. The common service evaluates identity and policy; each protocol adapter encodes
its own normative wire result.

## Upstream authentication

An Org may configure eligible upstream connectors, but their verified `(identity_namespace,
subject)` maps into the **global** Account directory. A namespace derived from an upstream issuer
or stable non-OIDC provider trust domain must be consistent across Org connector instances; an Org
ID or connector-row ID cannot turn the same external identity into multiple Accounts. Connector
configuration selects which source may be used for a given interaction, not which Account owns a
subject.

Upstream adapters verify protocol evidence before returning a normalized identity, profile
snapshots, verified-identifier assertions, authentication method/strength, and trusted group
assertions where supported. OIDC uses verified issuer + `sub`; SAML2 requires a trusted,
persistent subject contract; LDAP uses an immutable directory object identifier rather than a
renamable DN or display name. Generic OAuth requires a configured trusted identity endpoint with
stable subject provenance; access-token possession or a matching email alone is not identity
proof. Account resolution/linking and browser session creation use the shared identity service.
Matching email never auto-links Accounts or enrolls an Account into an Org. Invitation, Management
API provisioning, or explicitly authorized enterprise JIT creates
Membership separately. Raw directory or IdP groups map to the target Membership's local Org Groups
only under explicit connector policy; they do not grant an Application directly.

The validated interaction retains its client/Org, browser binding, nonce/state or protocol replay
fence, expiry, and intended authorization. After Account authentication, the shared service checks
active Org, Membership, Groups, Application admission, consent, and audience, then resumes that
interaction. Upstream credentials/assertions are never forwarded as AsterGate tokens.

## Downstream issuance

OIDC OP and OAuth Authorization Server are protocol projections over **one** client registry,
authorization request/consent and grant service, authorization-code store, token/key service,
browser session, refresh family, revocation, and introspection. OIDC adds ID-token claims,
UserInfo, discovery, and OIDC logout rules; it does not implement a second code exchange or token
issuer. OAuth resource scopes/audiences and OIDC identity scopes are distinguished within that
shared grant, and metadata advertises only implemented profiles.

`client_id` is unique under AsterGate's issuer and resolves an Application in exactly one Org.
Authorization requires the global Account's active Membership in that Org and the Application's
Group admission policy. The approved Org, Membership, client, scopes, resources, audience, nonce,
session, and grant are bound to the single-use code and resulting token family. Refresh repeats
authoritative Account, Membership, Group, client, grant, and audience checks. One token is not
relabeled or replayed into another Org or audience: Account-area Org switching starts a fresh
authorization and issues new credentials.

Downstream SAML2, when supported, uses the same Account/session and tenant admission decision and
its own signed assertion, audience, ACS, replay, metadata, and logout rules. SAML login does not
mint OAuth tokens; any explicit token exchange would require its own contract and the shared
token service. Machine-client grants use the same Org/Application authorization and token core
but do not invent a human Account or Membership. A resource server still enforces its own
business-domain permissions; a projected `groups` claim is a per-Application, allowlisted view of
the target Membership, not an independent authorization source.

## Forge and product ownership

| Concern | AsterForge | AsterGate |
| --- | --- | --- |
| Shared catalog/ports | reusable descriptor, typed registry/config validation and capability mechanics when genuinely product-neutral | assembled registry, enabled kinds/profiles, admin projection and permission checks |
| Upstream OIDC/OAuth | existing RP-side driver, discovery, PKCE, callback exchange, verification and profile normalization | Org connector instances, encrypted credentials, bound interaction, Account linking, Membership/Group policy |
| Upstream LDAP/SAML2 | new product-neutral wire/crypto/transport mechanics or crates where reuse is justified | source trust policy, namespaces, provisioning, mapping, session, audit, errors |
| Downstream OIDC/OAuth/SAML2 | reusable parser, JOSE/XML-signature or wire primitives where proven appropriate | OP/AS protocol policy and adapters, one client/grant/code/token core, subject, tenant policy and migrations |

`aster_forge_external_auth` currently models **upstream RP** OIDC/OAuth only. Its
`ExternalAuthProviderDriver` requires `start_authorization`, `exchange_callback`, and
`test_provider`; its enum and default feature-gated registry contain no LDAP, SAML2, or downstream
OP/AS. Its `ExternalAuthProfile` has namespace/subject/profile fields, not a verified group list;
the config's reserved `groups_claim` does not implement group synchronization. Extend or split
Forge crates to expose family-appropriate, product-neutral interfaces and normalized group
evidence where needed. Do not claim these new capabilities already exist, copy the RP driver as
an OP, or move AsterGate Account, Org, Membership, Group, grant policy, storage, migrations,
audit, or UI into Forge. New shared code requires its own default/feature-matrix tests and a clean
AsterGate build
against the published Forge revision, not merely an unpublished sibling checkout.

AsterDrive's external-auth integration supplies the existing pattern: Forge descriptors/registry
and OIDC/OAuth drivers; Drive-owned provider rows, callback flow, user resolution, MFA, cookies,
and audit. Its optional verified-email auto-link and auto-provision behavior is **not** AsterGate
policy. AsterDrive's storage Connector separately demonstrates descriptor/capability/config
discovery with product-owned lifecycle; it is not an authentication trait to import wholesale.

## Adding a provider versus adding a protocol

**A provider on an existing family** supplies a stable kind, descriptor/config schema, family
adapter and claim/trust normalization, feature registration, credential redaction, connection
test, and UI/catalog metadata. It uses the existing family wire tests and the same Account and
tenant services. Validate provider-specific issuer/subject and verified-email/group semantics,
disabled/unknown registration, callback/replay and error classification, and client integration.
No new token issuer or Account table is created.

**A new protocol family** first defines a normative wire/security profile and typed family port;
then adds product-neutral parsing/verification/signing mechanics where appropriate, descriptor
capabilities, registry dispatch, AsterGate route and interaction/response adapters, persisted
configuration migration, operational key/certificate lifecycle, and conformance/real-client
tests. LDAP bind and SAML assertion flows have their own replay, trust, and failure models. A
downstream family must reuse the shared identity/admission/authorization decision, and any OAuth
or OIDC extension must reuse the common client/grant/code/token/session/revocation core. A new
family cannot silently change the canonical issuer or existing subject identity.

## Security and compatibility gates

- Upstream: enforce TLS/certificate trust, SSRF and DNS/redirect limits on HTTP metadata, LDAP
  filter/transport safety, SAML XML signature/wrapping/recipient/audience/in-response-to/replay
  checks, OIDC issuer/audience/nonce/time/algorithm, OAuth profile subject provenance, bounded
  responses and timeouts. No unverified email or raw group assertion grants Account linkage or
  Membership. Store short-lived flow secrets hashed where lookup permits; redact credentials,
  assertions, codes, tokens, and diagnostic bodies.
- Downstream: test exact redirect/ACS/logout URIs, PKCE, state/browser binding, nonce, audience,
  issuer, subject, signature/key rotation, authorization-code single use, refresh reuse, consent,
  revocation and cross-Org denial. Concurrent consume/rotation has one winner on each supported
  database. Test a second Org and audience on the same Account: the old token must be rejected at
  the new target, while validity at its original audience follows its expiry/revocation policy.
- Lifecycle: cover descriptor/schema version and kind/protocol immutability where referenced,
  missing feature/unknown kind rejection, secret rotation, migration/rollback, removal with active
  clients or identities, discovery truthfulness, independently tested client interoperability,
  official conformance where available, and documented behavior for unsupported profiles.
- Failure: dependency outage, timeout, certificate/key rollover, stale metadata, duplicate
  subject, disabled Account/Membership/Org/client/group, and callback replay fail closed without
  creating a second identity fact or issuing partial credentials. Audit redacts secrets and
  identifies direction, kind, Org, interaction, and outcome without leaking subject/profile data.

## Evidence

Design references from the inspected sibling checkouts (not claims about AsterGate implementation):

- AsterForge `6c82e54`: `crates/aster_forge_external_auth/src/{driver,registry,types,providers}/`
  and `docs/crates/aster_forge_external_auth.md`.
- AsterDrive `cb4c0874`: `src/services/auth/external/{providers,login,resolution}.rs`,
  `developer-docs/zh-CN/design/external-auth.md`, and `src/storage/connectors/{contract,mod}.rs`.

The current AsterGate checkout contains only the generated health/frontend service foundation;
this RFC is a final target contract, not implementation or conformance evidence.
