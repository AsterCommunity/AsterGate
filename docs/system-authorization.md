# System Authorization

## Ownership

Resource authorization is tracked by [AG #8](https://github.com/AsterCommunity/AsterGate/issues/8).
Durable grant/revoke audit, operator attribution and delegation are tracked separately by
[AG #12](https://github.com/AsterCommunity/AsterGate/issues/12).

`SystemPermission` is the closed set of operations implemented by instance administration.
`src/services/authorization` owns evaluation; Account is the authenticated subject.
`system_permission_grants` owns assignments. System permissions grant no Organization, Application,
Group admission or resource-audience access.

The stable format is `astergate.system.<resource>.<action>`. Matching is exact and case-sensitive;
prefixes and wildcards never authorize. Future Application-defined keys use a validated
`PermissionKey` and separate scope/resource contracts. The `astergate` namespace is reserved for
registered product permissions. Unknown product keys, wildcards, malformed/oversized keys are
rejected. This parser implements no custom permission storage or authorization.

## Registry

| System Permission | Authentication | Last Active Holder Protection |
| --- | --- | --- |
| `astergate.system.accounts.read` | Active session | No |
| `astergate.system.accounts.create` | Active session | Yes |
| `astergate.system.accounts.suspend` | Current password | No |
| `astergate.system.accounts.restore` | Current password | No |
| `astergate.system.accounts.delete` | Current password | No |
| `astergate.system.settings.read` | Active session | No |
| `astergate.system.settings.update` | Current password | Yes |

`SystemPermission::ALL` and `definition()` define resource, action, authentication requirement and
last-holder policy. System checks require this enum rather than strings. `SystemPermissionValue`
is only the ORM encoding; `SystemPermissionDto` owns the HTTP enum. Tests compare all registered,
stored, serialized and OpenAPI values. The frontend uses generated types and exhaustive constants.
Adding an operation is explicit and cannot silently expand a `.manage` grant.

## Persistence

Grants store `(account_id, permission_key, created_at)` with a composite primary key, Account foreign
key and a 128-character permission column. Duplicate inserts retain the original timestamp. Setup
assigns the seven current permissions atomically; registration/provisioning assigns none. Erasure
removes grants. Suspension retains grants but makes them ineffective until restoration and sign-in.

The final schema lives in `m20261003_000001_account_lifecycle`; no new migration is registered.
Development databases which applied the earlier capability schema are not automatically converted
or reset. Rebuild them explicitly or provide a later data-preserving migration before use.

Public grant/revoke APIs, roles, delegation and authorization audit lifecycle are not implemented.
Repository grant writes are internal setup/test mechanics. Audit record improvements are a separate
follow-up; Account lifecycle still writes its existing transactional audit facts.

## Checks

Use cases explicitly declare their exact permission. Transport authenticates and parses only; URL
substrings never select privileges. Reads consult the writer DB for active Account, live owned
session and exact grant. Permission snapshots in frontend/session never authorize; unknown persisted
permission keys fail closed.
The accepted JWT session design in [RFC 0009](rfcs/0009-first-party-jwt-sessions.md) preserves these
checks: an access JWT identifies a live Account/`sid`, while services still read current exact grants.
`system_permissions` remains a frontend query projection. JWT claims never replace transaction-time
permission/session revalidation. Current authentication is still opaque; JWT integration is pending.

Writes check permission and the registry's authentication requirement, obtain an internal
single-use `SystemActionPermit`, acquire the shared system mutation lock, recheck Account/session/
grant and claim the captured Account revision in the same transaction. Concurrent permission or
session revocation, password changes and stale/replayed snapshots prevent the mutation. Creation
uses the same recheck. Settings update requires the update permission independently of read.

Suspension/erasure must leave an active holder of both critical permissions (`accounts.create`,
`settings.update`). Check and mutation share the system lock, preventing concurrent operators from
removing each other. A read-only holder can be removed. Self-deletion uses its own password/session
checks and this same last-holder rule.

## Acceptance

`tests/accounts/permissions.rs` covers the seven-operation Router matrix, password step-up, forbidden
side effects, exact/reserved identifier validation, stale grants/sessions, unknown persisted keys,
foreign keys, duplicate grants, self-escalation denial and concurrent operator removal.
`src/services/authorization/tests.rs` deterministically checks revocation between preflight/commit,
revision replay, suspended Accounts, expired sessions and identity mismatch. Platform tests cover
schema uniqueness/ownership/restart; OpenAPI tests cover wire enum and response schemas. Frontend
tests and Playwright cover exact per-operation controls and the actual Account/SMTP workflows.
