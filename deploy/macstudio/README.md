# Mac Studio deployment

This deployment runs the Buzz relay, PostgreSQL, and Redis on the company Mac
Studio. Media is stored in external S3-compatible storage; MinIO is deliberately
not part of the production stack.

## One-time setup

1. Review the FileVault and macOS application firewall warnings. They are
   recommended hardening controls, but remain advisory on a shared host where
   pre-boot unlock or inbound filtering could interrupt existing bots and VMs.
   Record the accepted operational risk when either control remains disabled.
2. Connect the Mac Studio and network equipment to a UPS.
3. Reserve at least 500 GB of disk and 8 GB of reclaimable memory for Buzz.
4. Install and start OrbStack or Docker Desktop.
5. Clone `git@github.com:sipherxyz/buzz.git`, check out `main`, and keep the
   checkout dedicated to deployment.
6. Copy `.env.example` to `.env`, fill the production values, and run
   `chmod 600 .env`.

Port 443 is already used by the existing HTTPS proxy on the assessed Mac
Studio. Keep Buzz on its configured internal port and route the company DNS or
Cloudflare/Tailscale ingress to that port; do not replace the existing port-443
process as part of this deployment.

## Deploy

After a release PR has merged `develop` into `main`:

```bash
ssh mac-studio
cd /absolute/path/to/buzz
./deploy/macstudio/deploy.sh --dry-run
./deploy/macstudio/deploy.sh
```

The script requires a clean `main`, fetches and fast-forwards from
`origin/main`, builds `sipher-buzz:<12-char-commit>`, takes a PostgreSQL backup
when a database already exists, starts the stack, and records the healthy
commit under the ignored `deploy/macstudio/state/` directory.

Preflight continues past FileVault and macOS firewall findings with `WARN`
output. A dirty/non-main checkout, insufficient resources, invalid production
configuration or external S3, and an unavailable Docker engine remain hard
failures.

Define each key only once in `.env`. Preflight rejects duplicate assignments
before reading any values because Compose resolves duplicates using the last
assignment, which would otherwise make the reviewed and deployed configuration
ambiguous.

## Closed-relay rollout

Keep `BUZZ_REQUIRE_AUTH_TOKEN=false` and
`BUZZ_REQUIRE_RELAY_MEMBERSHIP=false` through the initial deployment. Turning
on membership before the owner and employee identities are established can
lock existing clients out. These controls are read from `.env` through the
Compose service `env_file`; edit that reviewed file rather than using one-off
shell overrides during a cutover.

1. Generate and persist a stable 64-character hex `BUZZ_RELAY_PRIVATE_KEY`,
   then uncomment its setting in `.env`. Rotating it later changes the relay
   identity.
2. Uncomment `RELAY_OWNER_PUBKEY` in `.env` and set it to the 64-character hex
   pubkey of a human owner.
3. Deploy once with both enforcement flags still `false` so the relay can
   bootstrap the production community and owner without changing current
   access.
4. Mint employee invites and test redemption with the Sipher desktop build
   against `wss://buzz.sipher.gg:8443`.
5. Set `BUZZ_REQUIRE_AUTH_TOKEN=true`, deploy, and validate employee clients.
6. Set `BUZZ_REQUIRE_RELAY_MEMBERSHIP=true` only after invite, authentication,
   and client validation succeeds.

## Roll back

```bash
./deploy/macstudio/rollback.sh
```

This restarts the previous commit-tagged image. It does not automatically
restore PostgreSQL because a schema rollback is a separate, destructive
operator decision. Database dumps are retained under the ignored
`deploy/macstudio/backups/` directory and should also be copied off-host.

The automated deploy script refuses releases that add or change files under
`migrations/`. Those releases require a separately reviewed database
migration/rollback runbook; this keeps the normal image rollback compatible
with the current schema.

## Verification

```bash
docker compose --env-file deploy/macstudio/.env \
  -f deploy/macstudio/compose.yml ps
curl -fsS http://127.0.0.1:3000/health
```
