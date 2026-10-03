# RFC Implementation Matrix

This matrix describes code and reproducible evidence in this checkout. RFC proposals are not
implementation evidence. A narrow slice does not establish the complete RFC capability.

Baseline audit: `773b3cc` (2026-10-03). Earlier identity, protocol, connector, and key-lifecycle
claims referenced absent code and tests and have been removed.

Module delivery is tracked by [AG #1](https://github.com/AsterCommunity/AsterGate/issues/1).
Current Account/configuration evidence includes unmerged local work; checked tasks do not imply
GitHub main or a released deployment already contains that implementation.

| Contract area | Current evidence | Status |
| --- | --- | --- |
| Global Account lifecycle and local authentication | Product migration, typed domain/repositories, Account/system-permission REST routes, hosted bilingual UI, encrypted mail outbox, generation/configuration/architecture contracts; `tests/accounts/` covers the basic lifecycle | Implemented basic slice |
| System authorization | Seven exact operation permissions; registry/ORM/DTO/OpenAPI parity, least-privilege Router matrix, deterministic commit rechecks, last-holder concurrency tests; `docs/system-authorization.md` | Implemented basic slice |
| First-party JWT session architecture | Accepted [RFC 0009](0009-first-party-jwt-sessions.md); AD-style access/refresh JWTs, live `sid`/security epoch, exact permissions and [AF #42](https://github.com/AsterCommunity/AsterForge/issues/42) ownership; current code still uses opaque cookies | Accepted design; implementation pending |
| Organizations, memberships, groups and admission | RFC design only; no tenant entities or authorization implementation | Missing |
| Applications, API resources, roles and permissions | RFC design only; no product schema or services | Missing |
| Account/setup bootstrap | Atomic first Account, seven system permissions and typed system configuration; setup tests | Implemented basic slice |
| Instance branding and multiple public sites | Editable name/description and ordered trusted site URLs; first site is primary for mail; host-only cookies; revision conflicts and rollback; `tests/accounts/sites.rs` | Implemented basic slice |
| Public frontend configuration | Anonymous `/api/v1/public/frontend-config` returns branding/setup/Account policies without SMTP credentials; hosted UI and generated DTOs | Implemented REST slice |
| Runtime product configuration | Domain normalizers/dependencies, cached authentication/branding, lazy SMTP decoding, post-commit publication and supervised peer reload; `tests/configuration/` | Implemented integration slice |
| Configuration sync transport | Real isolated Redis verifies initial reconciliation, committed publication and independent-replica refresh; opt-in `ASTER_TEST_CONFIG_SYNC_REDIS=1` | Passed integration slice |
| Recovery from failed reload with no new event | [AF #38](https://github.com/AsterCommunity/AsterForge/issues/38) owns retry/reconciliation; AG does not add a duplicate periodic worker | Pending upstream |
| Authorization audit and delegation | Lifecycle audits are transactional, but actual operator attribution, grant/revoke audit events and delegated grant-management APIs remain follow-up work | Partial |
| Canonical issuer identity lifecycle | No persisted OIDC issuer or key lifecycle implementation | Missing |
| OIDC Authorization Code + PKCE S256 | No authorization/token endpoints, grant/code storage, or protocol tests | Missing |
| Discovery, JWKS, UserInfo and protocol logout | No provider routes or signing-key implementation | Missing |
| Refresh rotation, reuse detection and revocation | No OAuth grant or refresh-token implementation | Missing |
| Permission projection | No membership/group/role/resource projection implementation | Missing |
| Downstream plugin registration | No connector registry or downstream protocol adapter | Missing |
| Signing-key rotation, overlap, retirement and compromise | No key schema, lifecycle service, or tests | Missing |
| Device authorization grant | No device flow or protocol implementation | Missing |
| MFA, recovery codes, WebAuthn and passwordless | No factor schema or authentication implementation | Missing |
| Upstream OIDC/OAuth/LDAP/SAML federation | No upstream connector implementation | Missing |
| OpenAPI and frontend generated types | `tests/generate_openapi.rs` includes Account/operator security and write headers; generated JSON/SDK, frontend check/build | Implemented REST slice |
| Runtime, health and infrastructure migration | Forge runtime; `tests/platform/health.rs`, `tests/platform/migrations.rs` | Foundation slice |
| Shared HTTP middleware integration | Request ID/security headers/optional metrics currently installed; full Axum CSRF/CORS/trusted-peer/rate-limit integration tracked by [AG #4](https://github.com/AsterCommunity/AsterGate/issues/4), pending [AF #41](https://github.com/AsterCommunity/AsterForge/issues/41) | Partial; expanded integration pending |
| Shared REST envelope integration | Product REST wrappers exist; adopting Forge's shared typed envelope and regenerating Account/SDK contracts is tracked by [AG #14](https://github.com/AsterCommunity/AsterGate/issues/14), dependent on [AF #35](https://github.com/AsterCommunity/AsterForge/issues/35) | Pending upstream/product integration |
| SQLite Account/platform/configuration acceptance | 49 Account/site tests, 20 platform tests, 12 configuration tests; 10 library and 3 OpenAPI tests; Redis test additionally run with opt-in enabled | Passed basic slice |
| PostgreSQL Account acceptance | 49 Account/authorization/setup/settings/site tests and 20 platform tests including migration up/down, opt-in `ASTER_TEST_DATABASE_BACKEND=postgres` | Passed basic slice |
| MySQL Account acceptance | 49 Account/authorization/setup/settings/site tests and 20 platform tests including migration up/down, opt-in `ASTER_TEST_DATABASE_BACKEND=mysql` | Passed basic slice |
| Hosted Account workflows and SMTP | 7 Playwright scenarios: direct setup, optional SMTP, full local Account lifecycle, read-only/create-only controls, live branding, hostname-isolated sessions and primary mail; desktop/mobile screenshots | Passed basic slice |
| Frontend contracts and tooling | 18 unit tests, generated OpenAPI types, TypeScript/Biome check and production frontend build | Passed REST/UI slice |
| Official OAuth/OIDC conformance suite | No provider implementation or conformance artifact | Missing |

Missing, proposed, partial, and unverified capabilities must not appear as supported in discovery
or product documentation. A database smoke test returning early because its opt-in variable is
absent does not establish database-backend acceptance.

The implemented Account slice is defined in [Account lifecycle](../account-lifecycle.md).
Runtime configuration and its recovery limits are defined in [Runtime configuration](../runtime-configuration.md).
HTTP middleware scope and upstream dependencies are defined in [HTTP middleware](../http-middleware.md).
It does not implement phone identifiers, import/bulk provisioning, tenant authorization, MFA,
external identities or OAuth/OIDC. Account management uses setup-created system permissions, not a
deployment management token. Local SMTP acceptance is not evidence of production mail deliverability.

## Module Tracking

| Module | Tracking issue |
| --- | --- |
| Global Account lifecycle, setup and instance configuration | [AG #2](https://github.com/AsterCommunity/AsterGate/issues/2) |
| Canonical issuer, subjects and signing-key lifecycle | [AG #3](https://github.com/AsterCommunity/AsterGate/issues/3) |
| Axum middleware, CSRF and HTTP security integration | [AG #4](https://github.com/AsterCommunity/AsterGate/issues/4) |
| First-party JWT sessions and refresh lifecycle | [AG #5](https://github.com/AsterCommunity/AsterGate/issues/5) |
| Organization, Membership, Group and admission | [AG #6](https://github.com/AsterCommunity/AsterGate/issues/6) |
| Application and OAuth client registration | [AG #7](https://github.com/AsterCommunity/AsterGate/issues/7) |
| API Resource, Scope, Role and permission projection | [AG #8](https://github.com/AsterCommunity/AsterGate/issues/8) |
| Interaction, consent, MFA and assurance | [AG #9](https://github.com/AsterCommunity/AsterGate/issues/9) |
| OAuth/OIDC provider and downstream token/grant lifecycle | [AG #10](https://github.com/AsterCommunity/AsterGate/issues/10) |
| Upstream federation and downstream Connector adapters | [AG #11](https://github.com/AsterCommunity/AsterGate/issues/11) |
| Authorization audit, delegation and administrative lifecycle | [AG #12](https://github.com/AsterCommunity/AsterGate/issues/12) |
| Operations, recovery, developer integration and release evidence | [AG #13](https://github.com/AsterCommunity/AsterGate/issues/13) |
| Shared REST envelope and Account/SDK contract integration | [AG #14](https://github.com/AsterCommunity/AsterGate/issues/14) |

Tracking an issue does not promote a Proposed RFC to Accepted or an unimplemented capability to
Implemented. Modules may split scoped child issues while retaining this ownership/dependency map.
