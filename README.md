# AsterGate

AsterGate is an independent, self-hosted identity platform. A logical instance owns one canonical
identity/signing domain. Accounts are global identities; Organizations will own tenant Memberships
and authorization. AsterDrive is an optional downstream consumer.

The first implemented product module is the **local Account lifecycle**: registration and email
activation, password sign-in, profile editing/export, email replacement, password change/recovery,
browser-session management, operator provisioning/suspension/restoration, and permanent account
erasure. OAuth/OIDC, Organizations, group admission, RBAC, MFA, passkeys and federation are proposed
capabilities and are **not implemented**.

The accepted next authentication design is AD-style access/refresh JWTs with durable sessions,
refresh rotation and immediate revocation. [RFC 0009](docs/rfcs/0009-first-party-jwt-sessions.md)
defines the contract and [AF #42](https://github.com/AsterCommunity/AsterForge/issues/42) tracks shared
mechanisms. JWT integration is pending; the current Account runtime still uses opaque session cookies.

See [system authorization](docs/system-authorization.md), [implementation evidence](docs/rfcs/IMPLEMENTATION-MATRIX.md),
[Account contract](docs/account-lifecycle.md), [runtime configuration](docs/runtime-configuration.md), and [architecture RFCs](docs/rfcs/README.md).
Large-module delivery and dependencies are tracked in [AG #1](https://github.com/AsterCommunity/AsterGate/issues/1).
Shared Axum middleware adoption is [AG #4](https://github.com/AsterCommunity/AsterGate/issues/4),
with its current/target boundaries described in [HTTP middleware](docs/http-middleware.md).

## Local Account Development

```bash
cd frontend-panel
bun install --frozen-lockfile
bun run dev:accounts
```

This starts the Rust backend, Vite frontend at **http://127.0.0.1:4173**, and a local SMTP capture
service at **http://127.0.0.1:4025**. It creates an isolated temporary SQLite database and performs
the same first-account setup flow used by the product. The inbox is local development infrastructure;
it is never registered in the AsterGate API.
Use `AG_DEV_PORT=5173 bun run dev:accounts` to select another frontend port.

For an existing configured backend, use `bun run dev`; `ASTER_DEV_BACKEND` can override the default
proxy target `http://127.0.0.1:3000`.

## Deployment

Build the embedded frontend before a release build:

```bash
cd frontend-panel
bun run build
cd ..
cargo build --release
```

Debug builds can use an isolated fallback when the frontend bundle is absent. Release builds require
the real bundle. The existing Dockerfile and Compose configuration build and embed this frontend.

Static configuration defaults to `data/config.toml`; use `ASTER_CONFIG` to select another path.
First startup backfills a deployment master key into that file with restrictive permissions.
The key is 64 hexadecimal characters, generated with `openssl rand -hex 32`; deployments can
inject it through `ASTER__SECURITY__MASTER_KEY`. Only 64-character hex keys are accepted.
Back up this key with the database.
Environment overrides use `ASTER__...`; setup stores product settings and the initial Account in the database.
Relative SQLite/temp/log paths are resolved against the configuration directory. Product migrations
run at startup after Forge's infrastructure migration.

Before enabling registrations, configure:

- Complete `/setup` with an ordered public site URL list, initial Account, registration and verification policies.
  SMTP can be skipped and added later from Instance settings.
- Production sessions use Secure, HttpOnly, SameSite=Lax host-only `__Host-ag_session` cookies.
- Account management uses the setup-created Account session plus exact `astergate.system.*` operation permissions;
  there is no separate management token.

Registration defaults to closed until setup enables it. Open registration without SMTP creates active
Accounts whose email remains unverified; recovery and verification mail stay unavailable until SMTP
is configured. Requiring email verification is a separate policy and requires SMTP.

Secret values are excluded from config serialization/debug output. Store them in environment/secret
delivery, not Git. Keep the encryption key stable across replicas and restarts. Configure a single
set of HTTPS public origins behind TLS termination. Site name, description and the ordered site URL list
are editable from Instance settings; the first URL is the primary address for account mail.
Preserve the original Host through proxies. Forwarded headers cannot select origins or email URLs.
Sessions are host-only; different hostnames require separate logins. Public sites are not OIDC issuers.
Enable Redis `config_sync` with a shared topic and unique runtime IDs for live settings refresh across replicas.

## Verification

```bash
cargo fmt --all -- --check
cargo test --locked --features openapi
ASTER_TEST_CONFIG_SYNC_REDIS=1 cargo test --locked --features openapi --test configuration redis_transport
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
ASTER_TEST_DATABASE_BACKEND=postgres cargo test --locked --features openapi --test accounts --test platform
ASTER_TEST_DATABASE_BACKEND=mysql cargo test --locked --features openapi --test accounts --test platform
cd frontend-panel
bun run generate-api
bun run check
bun run test
bun run build
bun run test:e2e
```

The database matrix creates disposable containers and requires Docker. Browser tests start isolated
backend/SMTP/frontend fixtures and exercise real HTTP and SMTP. Install the project's Chromium with
`bunx playwright install chromium`, or use an existing Chrome installation with
`AG_BROWSER_CHANNEL=chrome bun run test:e2e`.

OpenAPI generation uses `cargo test --features openapi --test generate_openapi`. Generated artifacts
are `frontend-panel/generated/openapi.json` and `frontend-panel/src/types/api.generated.ts`.
Debug builds with `openapi` expose `/api-docs/openapi.json` and `/swagger-ui/`.

`GET /api/v1/public/frontend-config` exposes only public branding and Account setup/registration/mail policies.
`/health` is liveness, `/health/ready` checks database/cache and runtime configuration validity, and optional `metrics`
enables `/health/metrics`. Readiness does not prove SMTP delivery or Account lifecycle acceptance.
