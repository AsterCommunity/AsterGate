# RFC 0001: AsterGate System Architecture

- Status: Proposed
- Date: 2026-09-13
- Owners: AsterGate maintainers
- Depends on: none

## Summary

AsterGate will be a Rust identity and access management service for AsterCommunity applications.
It will act as an OAuth authorization server and OpenID Provider (OP), provide hosted sign-in and
account experiences, federate upstream identity providers, and expose administration and
self-service APIs.

The target deployment is a **modular monolith** built on the existing AsterForge runtime. Protocol,
identity, authorization, administration, and bidirectional connectors are separate modules inside
one service and one transactional database. They are not separate network services. This keeps
authorization codes, sessions, identity linking, grants, and audit decisions inside explicit
transaction and side-effect boundaries while allowing later extraction only where operational
evidence warrants it.

## Motivation

AsterDrive currently authenticates users locally and can act as an OAuth/OIDC relying party (RP)
for external providers. That solves sign-in for one product, but it does not provide a central
issuer for multiple applications. Each product would otherwise repeat user lifecycle, client
registration, signing keys, sessions, MFA, consent, federation, and authorization policy.

AsterGate centralizes those responsibilities. AsterDrive, AsterYggdrasil, and future applications
become OAuth/OIDC clients and resource servers. They retain product data and product permissions
that cannot be represented safely as shared identity policy.

## Goals

- Provide standards-based browser, native, device, and machine authentication for multiple
  applications.
- Keep users, credentials, identities, sessions, applications, grants, and signing keys under one
  auditable security boundary.
- Support self-hosting with SQLite for local evaluation and PostgreSQL/MySQL for supported
  production topologies, following the repository's existing database matrix.
- Use one canonical OIDC issuer and signing domain per logical AsterGate instance. Independent
  identity domains use independent instances, not a Realm product entity (RFC 0007).
- Keep credentials, external identities, and browser sessions on a global Account. Accounts may
  join multiple organizations through memberships; tenant-owned groups bind memberships and
  control application admission and organization permissions.
- Reuse AsterForge infrastructure and upstream-provider mechanics while keeping AsterGate product
  policy in this repository.
- Give hosted UI, custom UI, SDKs, and automation the same service/domain contracts.
- Build security properties that are testable under replay, concurrency, rotation, expiry, and
  partial failure.

## Non-goals

- Cloning Logto's console, pricing model, cloud control plane, or connector count.
- Treating OAuth access tokens as a replacement for application-domain authorization.
- Shipping arbitrary administrator-supplied code in the authentication process in the first
  releases.
- Supporting implicit grant or resource-owner password credentials grant.
- Starting with microservices or an event-sourced identity store.
- Moving AsterDrive users, policy groups, workspaces, or file permissions into AsterGate.

## System context

```text
Browser / native app / CLI / device
              |
              | OAuth/OIDC authorization
              v
       +-------------------+
       |     AsterGate     |
       |-------------------|
       | hosted experience |
       | protocol engine   |
       | identity service  |
       | authorization     |
       | federation        |
       | admin/account API |
       +-------------------+
          |      |       |
          |      |       +---- mail / webhooks / audit export
          |      +------------ upstream LDAP/SAML2/OIDC/OAuth identity sources
          +------------------- database / cache
              |
              | signed tokens, userinfo, introspection
              v
     AsterDrive / other OIDC/OAuth or future SAML2 applications and APIs
```

## Architectural decisions

### One deployable, explicit modules

The service remains one deployable until scaling, isolation, or ownership evidence justifies a
split. Modules communicate through typed Rust interfaces and database transactions, not loopback
HTTP calls.

The target backend shape is:

```text
src/api/routes/*
  -> src/services/* use cases
  -> src/domain/* pure policy and state transitions
  -> src/db/repository/* atomic persistence
  -> protocol/crypto/external ports
```

Proposed product modules:

| Module | Owns |
| --- | --- |
| `identity` | global accounts, identifiers, credentials, external identities, recovery and verification |
| `interaction` | sign-in/sign-up/consent/MFA flow state and browser interaction lifecycle |
| `oauth` | clients, authorization requests/codes, grants, access and refresh token issuance |
| `oidc` | discovery, ID tokens, userinfo, subject identifiers, OIDC logout |
| `session` | browser and refresh-token families, revocation, device/session presentation |
| `authorization` | API resources, scopes, roles, permissions, policy evaluation |
| `organization` | tenant organizations, account memberships, groups, group membership, and JIT assignments |
| `connectors/upstream` | external source adapters and normalized identity evidence across protocol families |
| `connectors/downstream` | Application-facing protocol adapters over one shared issuance and authorization core |
| `keys` | asymmetric signing-key lifecycle and JWKS publication |
| `admin` | privileged management use cases and audit projection |
| `account` | end-user profile, identity, factor, grant, and session self-service |

Routes perform transport parsing, authentication guards, and response mapping. Services own use
case orchestration and transaction boundaries. Domain code owns testable transitions and policy.
Repositories own queries and conditional writes, but do not decide user-facing behavior.

### AsterForge boundary

AsterGate uses AsterForge directly for product-neutral mechanics:

- runtime lifecycle, health, shutdown, logging, metrics, and allocation;
- database handles, transactions, migration helpers, cache, and configuration synchronization;
- mail outbox, scheduled/background task mechanics, and audit-log storage mechanics;
- API helpers, validation, cryptographic primitives, and product-neutral upstream external-auth
  drivers; additional shared protocol-family crates may be added or changed under RFC 0006.

AsterGate retains:

- all identity and authorization entities and migrations;
- OAuth/OIDC server policy, shared client/grant/code/token/session/revocation core, token contents,
  consent, and protocol-specific error mapping;
- instance identity-domain and organization semantics;
- credential, MFA, session, federation, and account-linking product rules;
- product audit actions/details, API DTOs, UI behavior, and integration tests.

Reusable protocol parsing, verification, signing, or typed connector-family mechanics may live in
AsterForge when product-neutral and independently tested. AsterGate retains configured instances,
durable identity and authorization facts, product transactions, and audit. Shared descriptor,
registry, configuration, and capability discovery never require LDAP/SAML2 adapters to implement
OAuth callbacks. A thin wrapper that adds no policy, error mapping, metrics, or configuration is
not a useful abstraction. RFC 0006 owns the detailed two-direction contract.

### Database is authoritative; cache is acceleration

The writer database is authoritative for credentials, authorization codes, grants, token
families, sessions, key metadata, membership, permissions, and single-use flow state. Security
decisions must not depend on eventually consistent cache entries.

Cache may hold discovery documents, public keys, client projections, rate-limit counters where
loss has a defined fail-safe behavior, and other reconstructable data. Cache eviction must not
grant access or make a consumed credential valid again.

### Durable state machines, typed by purpose

Authorization, recovery, MFA, federation, device authorization, and verification flows have
different payloads and transition rules. Each receives a typed record and explicit transition
contract. AsterGate will not create one universal JSON `flows` table that becomes a second,
untyped application runtime.

Shared mechanics may define `pending -> processing -> completed/failed/expired/cancelled`, revision
checks, attempt budgets, and conditional consumption. Each owning service defines which
transitions and side effects are valid.

### Side effects follow committed state

Database state required to justify an email, webhook, token, or audit event is committed before
the external side effect unless the protocol explicitly requires an atomic response derived from
the same transaction. Outbox records that must be delivered are written in the transaction.
Failures are observable and retryable; no security-relevant work is silently detached.

### Public protocol is separate from product REST

OAuth/OIDC and future SAML2 endpoints use their normative content types, parameters, status codes,
headers, and error bodies. They do not use the ordinary AsterGate JSON response envelope.
Management, Account, and Experience APIs may use the product REST envelope and generated OpenAPI
client.

## API surfaces

| Surface | Audience | Authentication |
| --- | --- | --- |
| OAuth/OIDC protocol | registered clients and resource servers | protocol-defined client/user credentials |
| Future SAML2 application protocol | registered SPs | protocol-defined assertions and Account session |
| Experience API | hosted or custom sign-in UI | short-lived interaction handle plus browser binding |
| Account API | global Account self-service | Account session or dedicated Account audience token; step-up for sensitive actions |
| Management API | operators and automation | scoped admin user or machine token |
| Internal operations | probes and deployment tooling | network/operator boundary |

The hosted sign-in UI is a first-party consumer of the Experience API. It must not gain hidden
privileged routes that a custom UI cannot safely model.

## Runtime topology

The first supported topology is stateless HTTP replicas plus one shared database and optional
shared Redis-compatible cache. Any replica may complete a flow created by another replica.
Therefore:

- no protocol flow depends on process memory;
- conditional SQL updates provide one-time consumption and rotation fences;
- key material is loaded from encrypted persistent storage or an explicit external key provider;
- scheduled cleanup and rotation jobs use leases;
- readiness fails when the writer database or required key material is unavailable;
- an optional cache failure follows an explicit fail-open/fail-closed policy per use case.

SQLite is suitable for single-instance development and small self-hosting. Multi-replica
production requires a database and cache topology whose concurrency properties are covered by the
repository's backend matrix.

One logical instance may have multiple replicas. Independent identity domains use separate
instances with isolated identity state and keys; sharing infrastructure does not establish an
independent failure domain. RFC 0007 defines these deployment and federation boundaries.

The product is multi-tenant under each instance's one canonical issuer/signing domain. Global
Accounts own authentication state and may join multiple organizations. Organizations own applications,
groups, resources, and access policy; a Membership is an Account's tenant identity. `client_id`
selects the application and its organization, while group policy decides whether the membership
may enter. Switching organization in the Account experience requires a fresh authorization for
the target organization and audience, never relabeling an existing token. A single-organization
self-hosted deployment uses this same model.

## Extension model

The first extension points are typed Rust traits and descriptor-driven registries:

- upstream identity connector families (OIDC/OAuth, LDAP, SAML2);
- downstream Application protocol families (OIDC/OAuth, future SAML2);
- mail/SMS sender;
- signing key provider;
- webhook destination;
- optional risk signal provider.

Descriptors expose capabilities and required configuration to the admin UI. Protocol-family ports
are typed by their actual wire behavior rather than one universal OAuth callback trait. The server
remains authoritative for validation. Arbitrary scripts and dynamically loaded native plugins are
deferred because they turn console access into code execution and complicate deterministic
authentication.

## Observability

Every security-sensitive use case produces structured, redacted telemetry:

- low-cardinality metrics by outcome, grant type, connector kind, and error class;
- request and interaction correlation identifiers;
- immutable audit facts for administrative and user security actions;
- no passwords, authorization codes, token values, cookies, client secrets, MFA secrets, raw SAML
  assertions, or full upstream responses in logs or audit details.

## Consequences

### Benefits

- Strong transactions and fewer distributed failure modes during security-sensitive development.
- Clear separation between protocol, product policy, and reusable infrastructure.
- One identity platform can serve multiple Aster applications without importing their business
  models.
- Global Account and tenant Membership/Group ownership are explicit from the first schema; adding
  another organization does not duplicate credentials or upstream identities.

### Costs

- The monolith needs strict module ownership to avoid a single oversized auth service.
- Standards conformance and key/token lifecycle testing are substantial even for a narrow MVP.
- SQLite support constrains some concurrency implementations and requires backend-specific tests.

## Open questions

- Which audited Rust JOSE and OAuth server components satisfy the required algorithms and permit
  explicit storage/state-machine ownership? RFC 0003 requires a dependency spike before selecting
  them.
- The canonical AsterGate issuer URL must be fixed before issuing durable production subjects and
  tokens. A different issuer is a separate deployment or an explicit issuer migration, not a
  per-organization setting.

## Alternatives rejected

- **Copy AsterDrive authentication into AsterGate:** it produces another RP with local cookies,
  not an authorization server for other applications.
- **Start as microservices:** transaction and deployment complexity arrives before independent
  scaling or ownership needs exist.
- **Use email as identity:** email can change, can be absent, and has provider-specific trust
  semantics.
- **Put application permissions entirely in AsterGate:** shared scopes and roles belong here;
  resource ownership and business invariants remain in each application.
