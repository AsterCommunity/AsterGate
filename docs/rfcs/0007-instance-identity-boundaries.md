# RFC 0007: Instance Identity Boundaries and Independent Deployments

- Status: Proposed
- Date: 2026-10-02
- Owners: AsterGate maintainers
- Depends on: RFC 0001, RFC 0002, RFC 0003, RFC 0006
- Amends: RFC 0001, RFC 0002, RFC 0003, RFC 0005, RFC 0006

## Summary

One logical AsterGate instance owns one canonical OIDC issuer and one identity/signing domain.
Organizations are business tenants inside that instance. A separate identity domain is served by
an independently configured and operated AsterGate instance, rather than another realm inside the
same instance.

`Realm` may describe an identity domain in comparisons with other products, but is not an
AsterGate product entity. There is no realm CRUD, realm selector, or `realm_id` on identity or
tenant records. This RFC replaces the singleton Realm ID/lifecycle and `realm_configuration`
entity proposed in RFC 0002 with instance configuration and explicit operational procedures.

These are target contracts. This RFC does not claim that issuer configuration, independent
identity deployments, federation, or the acceptance gates are implemented in the current skeleton.

## Motivation

AsterGate's current model deliberately lets one Account join multiple Organizations without
duplicating credentials or browser sessions. Organization policy already separates Memberships,
groups, applications, connectors, and grants. Adding a realm per Organization would change this
identity model and introduce another selection and authorization boundary into every flow.

Independent identity systems do have legitimate needs: separate account directories, signing
keys, administrator access, retention rules, upgrades, and recovery. Separate instances make
those boundaries explicit using ordinary deployment and storage isolation. The current product
does not require a cloud control plane hosting large numbers of independent identity domains.

## Definitions

| Term | Meaning |
| --- | --- |
| Instance | One logical identity service with one configured issuer, an authoritative identity database, signing-key ownership, and operational lifecycle |
| Replica | One process/pod serving the same instance, sharing its authoritative state and issuer |
| Organization / Org | A business tenant with Memberships, applications, groups, and authorization policy inside an instance |
| Identity domain / realm | A descriptive trust boundary; in AsterGate it is owned by an instance, not a separate managed entity |
| Independent instance | A different identity service with its own issuer, identity state, keys, credentials, and operational configuration |

An instance may run several replicas behind a load balancer. Deploying more replicas increases
capacity or availability of the same identity domain; it does not create another one. Conversely,
two instances may run on shared infrastructure while retaining separate logical identity state.
Shared infrastructure does not establish independent failure or administrator-compromise domains.

## Decisions

### One issuer per logical instance

The canonical issuer is explicit instance configuration and must be stable before durable subjects
or tokens are issued. Discovery, protocol endpoints, token validation, signing keys, and subject
policy use that configured issuer. A request cannot select another issuer through an Org ID,
email domain, arbitrary `Host`, or forwarded header.

Ingress routes configured hostnames to the intended instance. Trusted proxy configuration and
host validation are explicit; neither proxy trust nor application URL generation may manufacture
a new issuer. A custom hostname must preserve the configured protocol identity or follow the
issuer migration contract; it is not a second issuer alias.

Signing keys rotate within an instance with explicit verification overlap and retirement policy.
Separate instances do not reuse signing keys or a writable key namespace.

### Organizations retain business tenancy

Accounts, credentials, external identities, and browser sessions are global **within an instance**.
They are not global across all AsterGate deployments. An Account may join multiple Orgs through
separate Memberships. Org-owned applications and their registered clients select tenant context;
Membership and Group admission remain mandatory as specified in RFC 0002.

An Organization cannot acquire its own issuer, signing-key domain, or independent Account
directory by changing its settings. A requirement for those boundaries selects an independent
instance. A one-Org instance uses the same schema and authorization model as a multi-Org instance.

### Independent deployments own independent security state

Independent instances require:

- distinct canonical issuers and independently managed signing keys;
- separate authoritative identity databases and database credentials, with no cross-instance
  queries or writable identity tables;
- separate session, interaction, grant, token, connector-secret, and encryption-key ownership;
- isolated cache namespaces and access controls, or dedicated caches where the shared service
  cannot enforce the required boundary;
- browser cookie scope and names that prevent one instance's session from being accepted by
  another; sibling-host deployments use host-only session cookies;
- separately scoped bootstrap/operator credentials, application registrations, secret references,
  backup destinations and access, and operational configuration.

A database server, cache service, cluster, or secret manager may be shared only when its access
controls preserve these boundaries. Shared root administrators and infrastructure failures remain
shared risks. Requirements for physical isolation, data residency, or independent availability
must specify dedicated infrastructure and failure domains rather than infer them from instance
names or Kubernetes namespaces.

Running two processes against the same identity database and issuer is replication. Giving them
different issuers while reusing identity state is unsupported unless a separate migration contract
explicitly defines that temporary state.

### Cross-instance sign-in is explicit federation

One instance may act as an upstream OIDC provider for another through the upstream Connector
contract in RFC 0006. Trust uses the configured upstream issuer and validated subject; the receiving
instance authenticates or links a local Account and issues its own downstream tokens.

This does not share Account IDs, credentials, browser sessions, or Org membership. Matching email
addresses do not automatically merge Accounts or enroll Memberships. Local linking, enrollment,
admission, and step-up policies still apply. Applications validate the exact issuer and audience
they registered for; a token from another instance is not interchangeable.

Federated sign-in does not imply global logout, immediate revocation propagation, or a shared MFA
assurance level. Each supported integration must document upstream session/revocation behavior,
local grant lifetimes, assurance mapping, and provider-outage recovery. Cyclic federation routes
must be rejected or bounded before they can produce redirect loops.

### Operational lifecycle replaces Realm lifecycle

Instance configuration owns issuer, subject policy, authentication/session/token defaults, and
global branding/locale defaults. Org overrides remain explicitly bounded. There is no standalone
Realm record or selectable Realm status; the persistence mechanism for mutable global settings is
chosen with the relevant implementation slice.

Bootstrap creates the instance's operator, initial Org, and required key/configuration state with
one-winner behavior across replicas. Replicas share durable flow state and fenced cleanup/key
rotation jobs, as required by RFC 0001. Independent instances bootstrap and schedule separately.

Provisioning automation supplies per-instance configuration and secrets. Operators must be able
to upgrade, back up, restore, and retire each instance separately. A restore preserves its issuer,
durable subject mappings, and the key material needed for existing verification contracts; it
must not accidentally activate a second writer for the same identity domain. Restoration of
consumed codes or revoked grants requires a documented recovery/revocation procedure rather than
silently making old credentials usable again.

Retirement stops new issuance, applies session/grant revocation policy, and retains public
verification material and audit records for their required safety windows. An unavailable writer
database or required signing material causes readiness failure, not fallback to another instance.

## Compatibility and migration

This decision amends proposed design documents and introduces no runtime or database migration.
It removes the proposed Realm entity while preserving the existing one-issuer and Account/Org
ownership contracts. Implementations must not add speculative `realm_id` columns or realm APIs.

Creating an independent instance creates a new issuer and identity namespace. Moving an Org or
Account between instances is not a database row reassignment: clients, `(iss, sub)` identities,
connector trust, account mappings, grants, and application-local identities require an explicit
migration plan. Sessions, authorization codes, and refresh tokens are not copied as live credentials
under a different issuer. Issuer changes and any temporary coexistence require their own public
compatibility, recovery, and rollback procedure before execution.

## Alternatives and consequences

| Approach | Assessment |
| --- | --- |
| One instance with many Orgs | Default when users share an identity system and need separate business authorization |
| Independent instances | Chosen when account directories, issuers, keys, or operational lifecycles must be separate |
| One realm per Org in one instance | Rejected: couples business tenancy to identity isolation and duplicates tenant selection |
| Many realms, each containing Orgs, in one instance | Deferred: valid for a hosted platform, but adds issuer routing and pervasive isolation obligations |

Independent instances cost more baseline resources and require deployment automation and fleet
observability. They reduce the amount of security-sensitive multi-domain routing inside the
identity service and allow separate upgrades and recovery. They do not automatically guarantee
infrastructure isolation or eliminate the need for tenant-reference checks inside each instance.

Revisit native multi-realm support only when a confirmed hosted-platform requirement needs many
independent identity domains and measured instance provisioning, resource, or fleet-management
costs justify it. A new RFC must specify account/session ownership, realm-versus-Org semantics,
issuer routing, key and storage isolation, administration, migration, recovery, and acceptance
evidence. Adding a realm discriminator alone is insufficient.

## Acceptance gates

These gates accompany the relevant identity/deployment implementation slices:

1. Two replicas of one instance complete each other's flows while single-use consume/rotation
   still has exactly one winner. Issuer metadata and key state remain consistent across replicas.
2. Two independent instances reject each other's tokens, sessions, interaction handles, codes,
   and refresh tokens. Tests include colliding local identifiers and signing `kid` values to
   prove that issuer and key provenance, rather than incidental uniqueness, enforce the boundary.
3. One Account can participate in multiple Orgs within an instance; inactive Memberships and
   cross-Org references are denied, and organization switching requires fresh authorization.
4. Host/forwarded-header and caller-supplied Org manipulations cannot change issuer or bypass
   registered-client tenant selection. Sibling-host cookies do not cross instance boundaries.
5. Independent database, cache, secret, and backup access controls deny unintended cross-instance
   access. Failure or rotation of one instance's keys does not select another instance's keys.
6. Optional federation validates issuer, subject, audience, nonce, signature, and local linking/
   admission. Same-email identities, redirect loops, provider outage, and documented logout/
   revocation limits are covered before the integration is advertised.
7. Bootstrap concurrency, independent upgrade/restore, issuer/subject preservation, recovery
   revocation, retirement, and shared-infrastructure failure limits have reproducible evidence.
