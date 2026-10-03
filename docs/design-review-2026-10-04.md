# Account And System Authorization Design Review

Reviewed against RFCs 0001, 0002, 0003, 0005, 0007 and 0008 after implementing granular system
permissions. This is an evidence-backed follow-up list, not a claim that the full identity platform
or the remaining findings are implemented. The review preserves the accepted direct Account setup,
optional SMTP, global Account/tenant separation, initial-schema development policy and seven
explicit system permissions.

## Findings In Priority Order

### P1: Administrative Audit Facts Do Not Identify The Actual Operator

[`account/lifecycle.rs`](../src/services/account/lifecycle.rs) writes every event with `user_id = 0`
and uses the target Account UUID plus a string such as `account_operator` in details. Suspension,
restoration, deletion and provisioning therefore cannot answer which logged-in operator performed
the action. Request/session correlation is also absent. Existing transactional audit-failure tests
prove rollback, not audit attribution. RFC 0001 requires immutable, redacted security facts; RFC
0002 requires administrative actions to use the same role/audit model as automation.

Next: compare AD product audit types and Forge UUID-compatible storage/adapters. Introduce a typed
actor (Account/System), target, action, required permission, outcome and request correlation. Keep
authorization assignment separate from audit evidence, and exclude passwords/tokens/IP trust
assumptions from unvalidated detail JSON. Permission grant/revoke lifecycle is currently absent;
when added it must update grants, protect critical holders and write audit facts atomically.
`system_permission_granted` belongs in durable audit rows with the actual actor and target, not only
ordinary runtime logs. This remains the next module, after configuration/Account closure.

### P1: Account Deletion Has Immediate Erasure But No Retention Workflow

[`account/lifecycle.rs`](../src/services/account/lifecycle.rs) immediately erases profile,
identifiers, credentials, sessions, challenges and grants. This is the current visible product
contract and was tested; it is not the broader RFC 0002 deletion/retention workflow. There is no
`pending_deletion`, grace period, reference check, anonymization distinction or cancellation.
Future Memberships, grants and audit references must not be blindly cascaded or silently orphaned.

Next: choose retention/recovery policy before adding tenant/protocol dependents; model requested,
revoked, retained and erased phases separately. Change user-facing deletion promises only with
explicit product approval and tests for references, races, retention and recovery.

### P2: Setup Completion Does Not Inspect Critical Operator Recovery State

[`system_setup.rs`](../src/services/system_setup.rs) rejects users with an incomplete setup marker,
but a `true` marker always reports ready. Last-holder protection covers normal lifecycle APIs;
manual grant corruption, an incomplete restore or future unaudited grant removal can still leave
an initialized instance without a working critical operator. There is no operator handover or
grant-management API yet, so ordinary users cannot become administrators through supported flows.

Next: audited grant lifecycle/ownership transfer and explicit recovery procedures. Do not reopen
anonymous setup when operators disappear. Separate product initialization from operational
recovery/health, consistent with AD's authoritative setup-state approach.

### P2: Master-Key Backfill Is Stable Sequentially But Not Fenced Across Processes

[`config/loader.rs`](../src/config/loader.rs) reads/edits/writes the configuration file directly.
Sequential restart and strict hexadecimal validation are tested, but simultaneous
processes sharing a new file can independently choose keys, and interrupted writes can truncate
the config. Multi-replica deployments currently require explicit injection of one shared key.

Next: cross-process file locking and atomic file replacement, or a deployment contract that always
requires injected shared secrets for that topology. Test concurrent first boot and interruption;
never regenerate a malformed existing key.

### P2: Frontend CSP Retains Template-Wide Permissions

[`routes/frontend.rs`](../src/api/routes/frontend.rs) retains `script-src 'unsafe-inline'` and
`connect-src` for arbitrary HTTP/HTTPS/WS/WSS origins. The Account frontend uses same-origin APIs;
these allowances are wider than its implemented requirement. This increases the impact of a
future content injection bug even though no such exploit is established by this review.

Next: a production CSP derived from real bundle requirements, nonce/hash where needed, and a
separate local development policy. Verify the embedded production frontend rather than relying on
Vite-only tests.

## Engineering Follow-Up

- Administration remains in `account/lifecycle.rs` beside authentication/self-service. Move privileged
  use cases to the RFC 0001 `admin` ownership boundary when grant/audit lifecycle is implemented;
  keep the common Account transaction rules shared rather than forwarding facades.
- Account pagination queries `total` in the route and uses a full-page length as a next-cursor hint.
  A service-owned result and Forge overfetch trimming would preserve transport/repository boundaries
  and avoid an unnecessary final empty page. Profile/grant projection currently performs per-Account
  queries; batch them when directory size/latency evidence warrants it.
- One shared system row serializes setup, provisioning, registration, settings writes and destructive
  administration. It proves correctness today but can become an instance throughput bottleneck.
  Measure before splitting locks, preserving the last-holder and settings-policy fences.

## Verified Boundaries And Remaining RFC Scope

The implemented system-permission model provides exact operation checks, Account/session/grant
revalidation, current-password step-up, revision fences, critical last-holder concurrency protection,
no privilege on normal registration, no arbitrary system grant input and no URL-derived privilege.
The real Router matrix runs on SQLite/PostgreSQL/MySQL; deterministic permit tests cover changes
between preflight and transaction. Seven browser scenarios include read-only/create-only controls,
multiple hostnames, live branding and primary mail links.

Org/Membership/Group admission, RBAC, OAuth/OIDC, issuer/key lifecycle, MFA/passkeys, custom permission
persistence, audited delegation and production disaster recovery remain explicit future slices.
Readiness, compilation or UI screenshots do not establish those RFC capabilities.

First-party JWT sessions are now the accepted design in [RFC 0009](rfcs/0009-first-party-jwt-sessions.md),
with reusable mechanisms tracked by [AF #42](https://github.com/AsterCommunity/AsterForge/issues/42).
The runtime still uses opaque cookies. Existing lifecycle evidence must not be relabeled as JWT,
refresh-rotation or replay-detection acceptance; those gates remain pending implementation.

## Resolved During Configuration Integration

- Settings now carry a resource revision, compared under the system transaction lock; a stale client
  cannot overwrite another operator's settings or preserved SMTP secret. Two distinct writers and
  rollback/conflict tests prove this boundary.
- Runtime configuration uses the Forge registry's domain normalization and dependency pipeline,
  independently validated cached authentication/branding projections, and lazy SMTP decoding.
  Config-read failure no longer forces a SMTP decryption on every authentication request.
- Readiness now validates cached configuration and ciphertext. Wrong keys/corrupt SMTP block mail and
  management settings while ordinary login/branding remain available; liveness is distinct.
- Setup/settings commit applies the local snapshot, publishes reload hints, and subscribes through
  Forge's reconnecting supervisor. Initial/recovered connections run authoritative reconciliation.
  [AF #38](https://github.com/AsterCommunity/AsterForge/issues/38) still owns retrying failed reload or
  reconciliation without a new notification. AG does not add a duplicate periodic worker or claim that
  recovery behavior is complete. Exhausted publish retries similarly require operator resynchronization.
