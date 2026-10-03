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

[Unreleased]: https://github.com/AsterCommunity/AsterGate/commits/main
