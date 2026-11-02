# Changelog

## [Unreleased]

### Added

- Global Account lifecycle with verified-email registration, local password sign-in, profile/export,
  email replacement, password change/recovery, owned-session management, operator provisioning,
  suspension/restoration and permanent personal-data erasure.
- Hosted bilingual account and management interfaces, generated REST API contracts, transactional
  audit/revocation, encrypted durable email delivery, bounded authentication attempts, and real
  router/SQLite/PostgreSQL/MySQL and browser/SMTP acceptance tests.
- Granular `astergate.system.*` read/create/suspend/restore/delete/update authorization, exact
  registry checks, transaction-time revocation/revision rechecks and critical last-holder protection.
- Isolated `bun run dev:accounts` environment with local SMTP capture and disposable credentials.
- Editable instance branding and ordered public sites with primary email links, host-only sessions,
  public frontend configuration, escaped runtime HTML/manifest and settings revision conflict detection.
- Domain-normalized Forge configuration registry, cached authentication/branding projections, lazy SMTP
  credential decoding, post-commit reload publication and peer subscriptions with connection reconciliation.
- Forge Axum double-submit CSRF with host-only token bootstrap/rotation/clearing, explicit runtime
  CORS policy, direct-peer/trusted-proxy resolution, and shared process-local IP/authentication quotas.
  HTTP/HTTPS loopback setup uses the same-host browser source without trusting forwarded headers.
  Explicit committed operations issue or clear session/CSRF cookies together. Early rejections retain
  Gate error envelopes, request correlation, security headers and Retry-After; allowlisted clients can
  read admission 429 responses while preflight still consumes quotas.

### Changed

- Account and management REST responses now consume Forge's controlled generic envelope directly,
  retaining Gate error codes, HTTP status/headers, boolean acknowledgements and redacted diagnostics.
  OpenAPI and the panel SDK use operation response types; browser error handling distinguishes
  expired sessions from failed credentials and treats Retry-After as advisory without automatic retries.

### Configuration

- Registration defaults to closed. Setup enables open registration independently of optional SMTP;
  unverified email cannot authorize recovery. Required verification needs SMTP. Production sessions require HTTPS/Secure host cookies.
- Account-management APIs use setup-created Account sessions with seven namespaced system permissions; OAuth/OIDC
  and tenant authorization remain unimplemented.
- Deployment master keys use strictly validated 64-character hex (`openssl rand -hex 32`).
- Runtime product settings live in `system_config`; obsolete single-site, setup-token, management-token,
  Base64 master-key and legacy frontend asset override contracts are unsupported.
- Failed reload recovery without a new event remains tracked by AsterForge #38; AG does not implement
  a separate periodic synchronization worker.
- HTTP CORS stays disabled by default and never permits cross-origin cookie credentials. Trusted
  proxies and nonzero general/authentication quotas are configured in `http_security`; durable Account
  brute-force budgets remain authoritative alongside process-local HTTP admission.

[Unreleased]: https://github.com/AsterCommunity/AsterGate/commits/main
