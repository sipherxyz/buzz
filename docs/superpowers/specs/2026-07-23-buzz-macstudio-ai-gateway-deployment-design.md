# Buzz Mac Studio and AI Gateway Deployment Design

## Purpose

This design defines how Sipher will maintain its Buzz fork, add first-class
AI Gateway support, publish Sipher-owned artifacts, and operate the Buzz relay
on the existing Mac Studio without losing the ability to integrate future
changes from `block/buzz`.

The design has three independently testable workstreams:

1. Git governance and upstream synchronization.
2. AI Gateway support in Buzz Desktop and `buzz-agent`.
3. Tagged relay releases and production operation on the Mac Studio.

## Goals

- Make `develop` the default integration branch in `sipherxyz/buzz`.
- Keep `main` as the production-ready Sipher branch.
- Preserve all Sipher modifications across upstream synchronization.
- Add an `ai-gateway` provider with live model discovery and no duplicate
  credential entry.
- Build Sipher-owned multi-architecture container images and desktop releases.
- Deploy immutable relay releases to the Mac Studio with health checks,
  backup, monitoring, and rollback.
- Avoid exposing AI Gateway credentials to the relay or persisting them in
  Buzz agent configuration.

## Non-goals

- Running LLM inference on the Mac Studio.
- Proxying employee AI Gateway traffic through the Buzz relay.
- Deploying Kubernetes or a permanent staging environment on the Mac Studio.
- Replacing AI Gateway provider routing inside Buzz.
- Making the Mac Studio highly available.

## Repository and Branch Model

The remotes retain separate responsibilities:

```text
origin   git@github.com:sipherxyz/buzz.git
upstream https://github.com/block/buzz.git
```

Branches have the following roles:

| Branch | Role | Deployment authority |
|---|---|---|
| `develop` | Default branch and integration target | Never deployed to production |
| `main` | Production-ready Sipher history | Source for release tags only |
| `feature/*` | Product changes based on `develop` | None |
| `fix/*` | Non-emergency fixes based on `develop` | None |
| `sync/upstream-YYYYMMDD` | One auditable upstream merge | None |
| `release/*` | Version and release metadata changes | Produces a tag after merge |
| `hotfix/*` | Emergency correction based on `main` | Produces a patch tag after review |

The normal flow is:

```text
feature/* ──PR──► develop ──release PR──► main ──tag──► immutable artifact
                      ▲
upstream/main ──► sync/upstream-YYYYMMDD
```

`main` and `develop` are protected against direct pushes, force pushes, and
branch deletion. Required CI checks and at least one approving review apply to
both. Release tags are immutable and may only be created by the release
workflow or an explicitly authorized release administrator.

### Upstream synchronization

Every synchronization starts from the current `origin/develop`:

```bash
git fetch origin --prune
git fetch upstream --prune
git switch develop
git pull --ff-only origin develop
git switch -c sync/upstream-YYYYMMDD
git merge --no-ff upstream/main
```

Conflicts are resolved only on the sync branch. The branch receives the full
CI suite and a dedicated PR into `develop`. The merge commit records the exact
upstream parent, so no additional vendor branch is required.

The following operations are prohibited on shared branches:

- Resetting `main` or `develop` to `upstream/main`.
- Rebasing or force-pushing `main` or `develop`.
- Reapplying Sipher changes as an undocumented patch stack.
- Merging upstream directly into production `main`.

After an upstream sync has passed integration testing in `develop`, it reaches
`main` through the next release PR. This preserves Sipher commits and makes
upstream regressions independently reversible.

## Release Identity and Artifacts

Sipher releases use names that cannot collide with upstream tags:

| Artifact | Tag format | Published output |
|---|---|---|
| Relay | `sipher-relay-v<semver>` | `ghcr.io/sipherxyz/buzz:<semver>` and digest |
| Desktop | `sipher-desktop-v<semver>` | Signed Sipher desktop bundle and updater metadata |

The repository variable `GHCR_IMAGE` is set to
`ghcr.io/sipherxyz/buzz`. Container metadata, Helm defaults, Compose defaults,
documentation, release conditions, and updater endpoints are changed from
`block/buzz` to Sipher-owned locations where the value is not already
configurable.

Pull requests build and test images without publishing them. A merge to
`develop` runs integration CI but does not move a production image tag. A
release merge to `main` creates the Sipher release tag; the tag publishes an
ARM64/AMD64 image and provenance. The Mac Studio deploys the image by immutable
digest, not by `main`, `develop`, `latest`, or a mutable semver alias.

The first implementation requires a GitHub identity with `Write` access to
`sipherxyz/buzz`. The current `hoangtan282` identity has read-only access and
cannot create branches, repository settings, packages, or pull requests.

## AI Gateway Integration

### Provider boundary

Buzz exposes `ai-gateway` as a first-class provider in the Desktop UI and
persisted agent configuration. `buzz-agent` accepts `ai-gateway` as an alias
that reuses its OpenAI-compatible HTTP transport. It does not add a second
provider-specific HTTP implementation.

For the initial release, all Gateway-backed models use:

```text
POST <resolved-gateway-base>/v1/chat/completions
Authorization: Bearer <resolved-key>
```

Protocol selection by individual model is outside the initial scope because
AI Gateway already translates the OpenAI-compatible request to its underlying
providers.

### Profile and credential resolution

The Tauri backend owns Gateway discovery. It:

1. Locates the `ai-gateway` executable.
2. Resolves the selected profile, defaulting to `prod`.
3. Runs the offline status command with a bounded timeout to resolve base URL
   and credential presence.
4. Reads the existing AI Gateway macOS Keychain entry using service
   `ai-gateway` and account `prod`, with the Gateway-compatible file store as
   fallback.
5. Returns readiness and non-secret profile metadata to the React UI.

Buzz does not copy the Gateway key into persona settings, its database, logs,
or relay events. The Tauri process injects the key into the managed
`buzz-agent` child process only for that process lifetime.

### Model discovery

When `ai-gateway` is selected, the Tauri backend requests:

```http
GET <resolved-gateway-base>/v1/models
Authorization: Bearer <resolved-key>
X-AI-Gateway-Models-Detail: full
```

The result captures model ID, display name, owner/provider, context length,
completion limit, supported parameters, input modalities, and thinking
capability. The model picker uses `display_name` with the model ID as fallback.

Discovery has a short in-memory cache, an explicit refresh action, and a
last-known-good result. AI Gateway does not fall back to Buzz's built-in model
catalog because the Gateway response is authoritative. If a saved model is no
longer advertised, Buzz preserves the saved value, displays a blocking
warning, and requires the user to select an available model before starting a
new turn.

### User-visible states

The provider UI distinguishes:

- Gateway executable missing.
- Profile not authenticated.
- Gateway unreachable.
- Model catalog loading.
- Model catalog available.
- Previously selected model unavailable.

The authentication action directs the employee to the existing AI Gateway
login flow. Buzz does not render a duplicate API key input for this provider.

## Mac Studio Production Architecture

The Mac Studio runs only the Buzz server stack:

```text
Employee Buzz Desktop
        │ WSS/HTTPS
        ▼
Named Cloudflare Tunnel
        │ localhost-bound origin
        ▼
Buzz relay container
   ├── PostgreSQL 17
   ├── Redis 7 with AOF
   ├── S3-compatible object storage
   └── local git scratch/cache
```

LLM traffic remains:

```text
Employee Buzz Desktop / buzz-agent ──► employee AI Gateway ──► LLM provider
```

The relay never receives an AI Gateway credential or LLM request.

### Container runtime

The production stack uses the existing OrbStack Docker engine and
`deploy/compose` definitions. A Sipher Compose override:

- Uses the Sipher relay image digest.
- Binds the relay origin to loopback instead of exposing it on every LAN
  interface.
- Avoids the Caddy override because port 443 is already occupied.
- Uses stable named volumes for PostgreSQL and Redis.
- Keeps health, readiness, and restart policies.
- Points media storage to an external S3-compatible bucket where available.

An external S3-compatible bucket is the recommended media source of truth
because the internal SSD is already 95% utilized. If external object storage
is not approved, a dedicated external SSD with monitored capacity is required
before production deployment.

OrbStack and the Compose project start through a user LaunchAgent after the
Docker engine becomes ready. Production secrets live in a root- or
service-account-readable environment file with mode `0600`; no secret is
stored in the repository.

### Network ingress

A named, managed Cloudflare Tunnel exposes the relay hostname over TLS without
binding local port 443. The existing ad-hoc tunnel is not reused. The tunnel
origin points to the loopback relay port, and only the tunnel and local health
checks can reach it.

Buzz relay authentication, membership checks, rate limits, and request-size
limits remain enabled. Cloudflare rules provide coarse abuse protection, but
an interactive Cloudflare Access login is not placed in front of the WebSocket
endpoint unless the Desktop client is explicitly enhanced to supply its
credentials.

### Host prerequisites and hardening

Production deployment is blocked until all of these conditions hold:

- At least 500 GB of internal storage is free, or Buzz data has a dedicated
  external volume and external object storage.
- At least 8 GB of sustained host memory headroom is available under normal VM
  load.
- The exposed VNC credential observed in process arguments is rotated and no
  longer passed on a command line.
- macOS security updates are applied.
- LAN exposure of SSH, VNC, SMB, Netdata, and other listeners is reviewed and
  restricted by host firewall or network ACL.
- The Buzz service account has only the filesystem and runtime permissions it
  needs.
- A UPS or an accepted power-loss risk is documented.

FileVault requires a separate operational decision because full-disk
encryption can prevent unattended recovery after reboot. Until that decision
is made, secrets and off-host backups must be encrypted independently.

## Deployment and Rollback

A production deployment performs these steps:

1. Verify the requested release tag and resolve its published image digest.
2. Confirm disk, memory, OrbStack, backup, and tunnel preflight checks.
3. Record the currently running digest and Compose configuration revision.
4. Back up PostgreSQL and stable relay secrets.
5. Pull the new image digest.
6. Run database migrations using the release image.
7. Reconcile the Compose stack.
8. Wait for `/_liveness` and `/_readiness`.
9. Run authenticated WebSocket, message, media, and agent-launch smoke tests.
10. Mark the release successful and retain the previous digest.

Rollback restores the previous image digest and Compose revision. Schema
changes must be backward-compatible for at least one release so image rollback
does not require an immediate database restore. A database restore is reserved
for destructive migration failures and follows the tested backup runbook.

## Backup and Restore

The backup set contains:

- PostgreSQL logical backup.
- Stable relay private key and HMAC secret.
- Compose environment and non-secret configuration.
- Git persistent data if the deployed version still treats it as durable.
- Object-storage versioning or a separate object backup policy.

PostgreSQL backups run daily and are encrypted off-host. Retention is seven
daily, four weekly, and twelve monthly restore points. Backup success is
monitored, and a restore drill is performed before the initial rollout and
quarterly afterward.

Redis is not the system-of-record backup source. Its AOF improves restart
continuity, while PostgreSQL, stable secrets, and object storage define the
recoverable state.

## Observability

The deployment exports relay metrics locally and integrates them with the
existing Netdata installation or a dedicated Prometheus scraper. Metrics and
admin ports are not publicly exposed.

Required alerts cover:

- Relay readiness and restart count.
- PostgreSQL and Redis health.
- Disk at 80% warning and 90% critical.
- Sustained memory pressure or swap growth.
- Backup failure or stale backup.
- Cloudflare Tunnel disconnect.
- Certificate/hostname and external WebSocket health.

Logs are structured, size-limited, and rotated. Secret values and authorization
headers are redacted.

## Testing Strategy

Git and release tests verify:

- Protected-branch and tag rules.
- Upstream merge rehearsal against `develop`.
- Sipher tag parsing and artifact naming.
- ARM64 and AMD64 image publication.
- Digest-pinned deployment and rollback.

AI Gateway tests verify:

- Provider alias resolution.
- Profile status parsing and timeout behavior.
- macOS Keychain and file-fallback credential resolution without logging keys.
- Base URL normalization.
- Detailed model response parsing.
- Authorization and model-detail request headers.
- Loading, offline, refresh, removed-model, and authentication UI states.
- A complete agent turn through a mock OpenAI-compatible Gateway endpoint.

Mac Studio validation verifies:

- Clean boot and automatic stack recovery.
- Backup and full restore into an isolated Compose project.
- Tunnel-only ingress.
- Authenticated relay connection, messaging, media, git, and agent launch.
- Rollback to the immediately previous image digest.

## Rollout

1. Establish GitHub permissions, `develop`, protection rules, and Sipher
   artifact ownership.
2. Implement and release AI Gateway support through `develop`.
3. Remediate Mac Studio storage, memory, credentials, patching, and firewall.
4. Build the Sipher relay image and validate it in an ephemeral local stack.
5. Perform backup and restore rehearsal.
6. Deploy a limited employee pilot.
7. Observe resource, reliability, and user metrics for two weeks.
8. Expand internal access only if the acceptance criteria remain satisfied.

## Acceptance Criteria

- `develop` is the default protected branch and `main` is the protected
  production branch.
- A rehearsal merge from `upstream/main` preserves all Sipher changes and
  passes CI.
- An employee already authenticated with AI Gateway can select it, refresh its
  advertised models, select a model, and complete an agent turn without
  entering another API key.
- No AI Gateway credential is present in Buzz persisted configuration, relay
  traffic, or logs.
- The Mac Studio deploys a Sipher release by digest and passes all health and
  smoke checks.
- The previous release can be restored without a database restore.
- A fresh environment can be recovered from the documented off-host backup.
- The pilot completes two weeks without critical disk, memory, tunnel,
  database, or backup alerts.
