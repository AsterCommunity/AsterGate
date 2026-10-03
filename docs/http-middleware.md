# HTTP Middleware Integration

- Product tracking: [AG #4](https://github.com/AsterCommunity/AsterGate/issues/4)
- Shared implementation: [AF #41](https://github.com/AsterCommunity/AsterForge/issues/41)
- Related JWT session integration: [AG #5](https://github.com/AsterCommunity/AsterGate/issues/5)

## Current Evidence And Dependency

AG currently uses Forge Axum request ID, security headers and optional metrics. Account writes use
the product's `X-AsterGate-Request: 1` requirement plus configured site/Origin/Host checks, and
authentication budgets are persisted in the DB. There is no Forge Axum double-submit CSRF, runtime
CORS, trusted-peer adapter or governor-based HTTP limiting wired into AG yet.

At this audit, remote Forge `master` and AG's lockfile use `1ba5754792ae94bd8888b03d09ada1644b2107c0`.
AF #41 is Open. The sibling Forge working tree has new shared/Axum middleware files under development;
that is not a published dependency or completed AG integration. AG consumes the tested published
Git revision once available, updates the lockfile and verifies its real request pipeline. It does
not use a permanent sibling path override or enable Actix to obtain a missing Axum adapter.

The currently pinned Forge default-security-header middleware can overwrite product headers,
including Account `Referrer-Policy: no-referrer`; AF #41 owns the correction and regression tests.
The existing Account layer sets the stricter policy, but that alone does not prove the final outer
response preserves it. Expanded integration must test the complete Router response.

## AD Reference And Ownership

AD uses Forge source validation and double-submit CSRF in Cookie-authenticated unsafe operations,
issues/clears CSRF Cookies with authentication Cookies, and keeps CORS/proxy/rate-limit selection as
product policy. Reuse those mechanisms through the framework-neutral core and Axum adapters, rather
than copying Actix-specific request types or maintaining separate verification algorithms in AG.

Forge owns shared token/source/header parsing, runtime CORS mechanics, trusted-peer extraction,
governor limiters, request correlation, default header behavior and HTTP metrics adapters. AG owns
route policy, trusted sites/proxies, session authentication, permissions, persisted Account budgets,
DTO/envelope/error mapping and frontend wiring. Shared middleware does not authorize an Account
or infer a system permission from a URL.

## Product Integration Contract

| Capability | AG integration and required boundary |
| --- | --- |
| Source validation | Resolve configured public sites and explicit request/peer policy; Origin/Referer/Sec-Fetch-Site handling follows the shared contract without trusting arbitrary forwarded Host |
| Double-submit CSRF | Check configured Cookie/header token names for Cookie-authenticated unsafe actions, including logout and future refresh; reject missing/duplicate/invalid/mismatched tokens with typed product errors |
| Anonymous setup/login/mail flows | Apply explicit source and custom-header protection even before a session/CSRF Cookie exists; specify anonymous token bootstrap if one is needed, rather than rejecting all first logins or exempting every auth route |
| Cookie/frontend lifecycle | Issue, rotate and exactly clear the readable CSRF Cookie along with protected session changes; access/refresh credentials stay HttpOnly; frontend sends the CSRF header only to configured first-party APIs |
| Runtime CORS | Default disabled for the current same-origin UI; enable exact allowed origins only for an explicit cross-origin surface; CORS never replaces CSRF or Application admission |
| Trusted client IP | Obtain direct socket peer explicitly; consult forwarded values only when that peer is trusted; absence of peer or a claimed X-Forwarded-For is not a trusted identity |
| HTTP rate limits | Apply selected IP/keyed quotas with product 429/Retry-After mapping; retain persisted HMAC-keyed Account/password/challenge budgets and their restart/concurrency guarantees |
| Security headers | Preserve product no-referrer/CSP and deliberate overrides; shared defaults fill missing headers rather than silently weakening a surface's policy |
| Request ID/metrics | Follow the shared bounded ID trust policy and actual layer ordering; no secret/high-cardinality labels; record both accepted and rejected requests |

CSRF setup and source validation are transport concerns. Pure policy/Service commands stay free of
HTTP types; protected use cases still verify live Account/session/grants and transaction revisions.
First-party JWT adoption retains these protections: token format does not determine whether a
Cookie-authenticated request needs CSRF validation.

Multiple public sites remain exact trusted Origins, not an automatic CORS wildcard or issuer alias.
Middleware replacement cannot silently widen accepted Origins, trust proxy input without peer
evidence, or turn process-local throttling into the authoritative Account security budget.

## Adoption And Acceptance

1. Consume AF #41's published adapter/core APIs and remove duplicated generic AG mechanisms only
   when the replacement preserves the current and explicitly adopted security contracts.
2. Wire resolver/state/predicate/error factories and direct-peer delivery into the real Axum stack;
   document layer order, endpoint policy and exclusion of protocol-specific surfaces where required.
3. Synchronize Cookie issuance/clearing, frontend header handling, error enums/OpenAPI/SDK and tests.
   No compatibility alias is required for unreleased AG, but first setup/login must remain usable.
4. Real Router tests cover safe/unsafe methods, token/source absence/duplicates/mismatch/size,
   Origin/Referer precedence, spoofed fetch metadata, custom names and structured JSON errors.
5. CORS tests cover disabled/pass-through, dynamic policy, preflight credentials/wildcards/headers,
   Vary and exact Origin schemes; proxy tests cover trusted/untrusted/missing peer and IPv4/IPv6.
6. Limit tests cover quota edges, normalized key isolation, concurrent reservations, rejection delay,
   no inferred trust from proxies, and retention of persisted Account security limits.
7. Full-stack tests verify no-referrer/CSP, bounded request IDs, low-cardinality success/error metrics
   and actual browser setup/login/management/refresh workflows with CSRF protection.
8. Axum-only dependency/feature checks exclude Actix. API generation, frontend checks and relevant
   Account/session/browser regression suites pass before the matrix marks complete integration.

Unpublished upstream files, a registered layer or current custom-header tests alone do not establish
completion. AF's feature/transport contract tests and AG's product acceptance are separate evidence.
