# Runtime Configuration

AsterGate follows AsterDrive's configuration boundaries. Deployment infrastructure and the master key
are loaded from TOML/environment. Setup creates the first management Account and initializes mutable
product settings in Forge `system_config`; there are no setup credentials or separate management token.

## Registry And Values

`src/config/definitions.rs` owns key constants and complete per-item Forge metadata. Domain normalizers
delegate to the same bounded branding, origin, policy, lifetime and SMTP constructors used by services.
Registration/delivery dependency validation checks the proposed value against the full candidate map.
Keys are constants; there is no parallel key enum or old key mapping.

Branding, site origins, Account policy and mail validation live in separate modules under
`src/services/system_settings/`. SMTP password input is validated before encryption. Its registry entry
describes encrypted storage; plaintext rules are not applied to ciphertext.

Settings reads include `revision`. Writes require that revision and current password, verify the exact
`astergate.system.settings.update` permission, and compare the revision under the system transaction
lock. Stale edits return `settings.revision_conflict` and must reload. Omitted/null SMTP passwords
preserve the prior secret only for a matching revision. Audit failure rolls back every setting and revision.

## Public Sites And Branding

`public_site_urls` stores a JSON string array; setup/settings DTOs use `site_urls`. Values are normalized,
deduplicated in order and bounded to 1-16 entries after setup. HTTPS is required except loopback HTTP.
Paths, queries, fragments, credentials and wildcards are rejected. Empty lists are allowed only as the
uninitialized registry default. The first entry is the primary address for mail; selecting another entry
as primary reorders the list. There is no `public_origin` field, key or compatibility parser.

All sites share the same name and description. Names/descriptions are trimmed and bounded to 120/300
UTF-8 bytes; blank values reset to product defaults. The anonymous `GET /api/v1/public/frontend-config`
query returns only branding and Account availability policies. Embedded HTML escapes runtime text,
React updates document metadata without HTML injection, and the manifest uses structured JSON with
relative start/scope URLs. Dynamic HTML/manifest are not precached by the service worker.

Mail links always use the current primary site at dispatch, regardless of request Host/Origin or
forwarded headers. Previously delivered links cannot be rewritten; retain the old hostname until
outstanding challenges expire when moving a primary site. Public sites do not define additional OIDC
issuers. The proposed canonical issuer lifecycle remains a separate RFC capability.

Browser writes require the custom request header and an exact configured Origin matching the
configured request Host. Proxies must preserve Host. Unconfigured/removed authorities are rejected.
Cookie security/name follows the request's configured site, so changing the primary does not change
cookies on other sites. Cookies are host-only, not port-specific; different hostnames require separate
logins. No cross-origin CORS or silent cross-domain SSO is added.

## Cache And Synchronization

An atomic Forge-backed snapshot provides independently validated branding and non-secret authentication
projections. Login/cookie/policy reads use this cache and do not load every config row or decrypt SMTP
credentials. Mail is decoded lazily once per snapshot using the deployment key. Corrupt SMTP ciphertext
blocks mail/management settings but leaves ordinary login and public branding available. Invalid security
policy fails closed. Readiness reports invalid configuration separately from liveness; it does not probe
SMTP deliverability. Settings transactions still read authoritative configuration and fence revisions.

After commit, setup/settings writes apply the complete snapshot locally and publish key-only reload hints
through Forge `ConfigSyncRuntime`. Peer subscriptions reload from the writer DB, including full
reconciliation after initial connection or reconnection. Old revisions cannot overwrite newer snapshots.
Shutdown cancels the Forge subscription worker. Publication retries are bounded; publication failure
never reports an already committed mutation as rolled back and is logged for operator intervention.

Single-process defaults disable Redis notifications. Multi-replica deployments need a shared Redis topic,
shared database/key, and unique runtime IDs. AG does not add a generic periodic reconciliation worker.
[AsterForge #38](https://github.com/AsterCommunity/AsterForge/issues/38) owns retrying failed reload or
connection reconciliation without another notification. That issue is still open: AG does not claim
automatic recovery from that failure or from exhausted publication retries. A new notification or
successful reconnect/startup reload is needed with the current Forge implementation.

## Evidence

`tests/configuration/` covers registry normalization/dependencies, database-independent hot reads,
group failure isolation, wrong keys, stale snapshot fencing, publication rollback boundaries, peer reload,
connection reconciliation and shutdown. `ASTER_TEST_CONFIG_SYNC_REDIS=1` enables a real disposable Redis
transport test. `tests/accounts/sites.rs` covers site order, primary mail, cookie boundaries, removal,
branding escaping, invalid/obsolete fields, concurrent writers and transaction rollback. Browser tests
exercise live settings, metadata, hostname-specific cookies and primary mail through local SMTP.
