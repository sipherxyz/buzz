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
- Build Sipher-owned desktop releases and reproducible local relay images.
- Deploy the reviewed `main` commit to the Mac Studio through SSH with health
  checks, backup, monitoring, and rollback.
- Avoid exposing AI Gateway credentials to the relay or persisting them in
  Buzz agent configuration.

## Non-goals

- Running LLM inference on the Mac Studio.
- Proxying employee AI Gateway traffic through the Buzz relay.
- Deploying Kubernetes or a permanent staging environment on the Mac Studio.
- Replacing AI Gateway provider routing inside Buzz.
- Making the Mac Studio highly available.
- Guaranteeing unattended recovery from an uncontrolled reboot while FileVault
  is locked at the macOS preboot screen.

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
| `main` | Production-ready Sipher history | Source for manual Mac Studio deployment |
| `feature/*` | Product changes based on `develop` | None |
| `fix/*` | Non-emergency fixes based on `develop` | None |
| `sync/upstream-YYYYMMDD` | Repo-owner upstream merge based on `main` | None |
| `release/*` | Version and release metadata changes | Produces a tag after merge |
| `hotfix/*` | Emergency correction based on `main` | Produces a patch tag after review |

The normal flow is:

```text
feature/* ──PR──► develop ──release PR──► main ──SSH pull/build──► Mac Studio
                      ▲                    ▲
                      │                    │
                      └──── forward PR ────┤
upstream/main ──► sync/upstream-YYYYMMDD ──PR
```

`main` and `develop` are protected against direct pushes, force pushes, and
branch deletion. Required CI checks and at least one approving review apply to
both. Desktop release tags are immutable and may only be created by the release
workflow or an explicitly authorized release administrator.

### Owner-initiated upstream synchronization

The repository owner initiates upstream synchronization only when Sipher needs
a specific upstream change. Every synchronization starts from the current
`origin/main`:

```bash
git fetch origin --prune
git fetch upstream --prune
git switch main
git pull --ff-only origin main
git switch -c sync/upstream-YYYYMMDD
git merge --no-ff upstream/main
```

Conflicts are resolved only on the sync branch. The branch receives the full
CI suite and a repository-owner PR directly into `main`. The merge commit
records the exact upstream parent, so no additional vendor branch is required.
Merging the sync PR does not deploy production because the Mac Studio accepts
changes only when an operator explicitly starts the SSH deployment.

After the sync PR is merged, the repository owner opens a second PR from
`main` into `develop`. The synchronization is not complete until that forward
PR is merged, ensuring future feature and release work includes the upstream
changes.

The following operations are prohibited on shared branches:

- Resetting `main` or `develop` to `upstream/main`.
- Rebasing or force-pushing `main` or `develop`.
- Reapplying Sipher changes as an undocumented patch stack.
- Merging upstream into `main` outside a repository-owner
  `sync/upstream-YYYYMMDD` PR.

Every sync PR records the old and new upstream commit SHAs, upstream release
notes reviewed, conflicts resolved, and Sipher-specific regression suites run.

An accepted `hotfix/*` PR targets `main`, produces a patch release, and is then
immediately forward-merged from `main` into `develop` through a second PR. The
hotfix is not considered complete until both PRs are merged, preventing the
next normal release from dropping the correction.

## Desktop Release Identity and Relay Deployment Source

Desktop releases use names that cannot collide with upstream tags. Relay
deployments use the reviewed Git commit as their identity:

| Artifact | Identity | Output |
|---|---|---|
| Relay | `main@<40-character commit SHA>` | Locally built `sipher-buzz:<short-sha>` image |
| Desktop | `sipher-desktop-v<semver>` | Signed Sipher desktop bundle and updater metadata |

Pull requests build and test the relay without publishing a production image.
A merge to `develop` or `main` does not deploy automatically. The deployment
operator connects to the Mac Studio, verifies that the production checkout is
clean, fast-forwards it to `origin/main`, records the exact commit SHA, builds
the relay image locally, and reconciles the Compose stack only after preflight
and backup gates pass.

The first implementation requires a GitHub identity with `Write` access to
`sipherxyz/buzz`. The current `hoangtan282` identity has read-only access and
cannot create branches, repository settings, packages, or pull requests.

### Desktop identity and distribution

The Sipher desktop fork uses:

```text
Product name: Sipher Buzz
Bundle identifier: xyz.sipher.buzz
Deep-link scheme: sipher-buzz
Keychain service: sipher-buzz-desktop
Updater endpoint:
https://github.com/sipherxyz/buzz/releases/download/sipher-desktop-latest/latest.json
```

Desktop release candidates use `sipher-desktop-v<semver>-rc.<n>` and are
distributed only to the pilot ring. Stable releases use
`sipher-desktop-v<semver>` and advance the Sipher updater manifest only after
pilot approval. macOS artifacts are signed and notarized with Sipher-owned
credentials held in GitHub Actions secrets or an equivalent protected signing
service.

The initial employee distribution is a signed DMG delivered through the
company software portal or MDM. The packaged app includes the managed
`buzz-agent`, the production relay URL, and the `ai-gateway` provider. Desktop
updates roll out in three rings: engineering pilot, wider internal pilot, then
all employees. A failed release is stopped by freezing the updater manifest;
because Tauri updaters do not provide a reliable downgrade path, recovery uses
a fixed forward release or an explicitly distributed previous signed DMG.

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

The initial compatibility baseline is AI Gateway commit
`a52bf5c80f7cbb61494ffcec1aac3a40cc536021` or a later build preserving its
profile, secure-store, `/v1/models`, and Chat Completions contracts. Unknown
status output, an unsupported profile format, or a newer incompatible
secure-store layout fails closed with an actionable compatibility message.
Parsing tests use captured outputs from every supported Gateway release. Buzz
prefers the Gateway's stable profile and secure-store contracts and uses
human-readable CLI output only where no structured contract exists.

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

Model discovery must complete within five seconds on a healthy local Gateway.
The in-memory result is cached for two minutes. A manual refresh bypasses the
cache, and a failed refresh may display the last-known-good catalog with a
stale warning but cannot silently validate a model that the current Gateway no
longer advertises.

### User-visible states

The provider UI distinguishes:

- Gateway executable missing.
- Gateway version unsupported.
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

- Uses the locally built relay image tagged with the deployed commit SHA.
- Binds the relay origin to loopback instead of exposing it on every LAN
  interface.
- Avoids the Caddy override because port 443 is already occupied.
- Uses stable named volumes for PostgreSQL and Redis.
- Keeps health, readiness, and restart policies.
- Points media storage to the production S3-compatible bucket.

An external S3-compatible bucket is mandatory as the media source of truth
because the internal SSD is already 95% utilized. The bucket has encryption,
versioning, public access blocking, lifecycle rules, and a credential scoped to
the single media bucket. MinIO is disabled in production. Local disks retain
only PostgreSQL, Redis, container layers, logs, and bounded git scratch/cache.

OrbStack and the Compose project run in a dedicated non-admin macOS service
account. A user LaunchAgent starts OrbStack and then starts the Compose project
after the Docker engine becomes ready. The account does not auto-login.
Planned restarts use authenticated restart so FileVault can return to the
service session without leaving the disk unencrypted. An uncontrolled power
loss requires an authorized operator to unlock FileVault and log in; this is
covered by the four-hour disaster RTO.

Production secrets live in a service-account-readable environment file with
mode `0600`; no secret is stored in the repository. The Mac Studio has a UPS
capable of orderly shutdown, and the on-call runbook identifies who can
perform a physical FileVault unlock.

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
- FileVault is enabled, a planned authenticated restart is tested, and the
  physical recovery procedure meets the disaster RTO.
- The UPS and orderly-shutdown path are tested.
- An external S3-compatible bucket passes read, write, delete, list,
  encryption, versioning, and restore checks.

## Deployment and Rollback

A production deployment performs these steps:

1. Connect to the Mac Studio through the configured SSH host.
2. Confirm disk, memory, OrbStack, backup, Git, and tunnel preflight checks.
3. Verify that the production checkout is on `main` and has no local changes.
4. Fetch `origin/main` and fast-forward with `git pull --ff-only`.
5. Record the previous and new 40-character commit SHAs.
6. Back up PostgreSQL and stable relay secrets.
7. Build `sipher-buzz:<short-sha>` from the checked-out source.
8. Run database migrations using the newly built image.
9. Reconcile the Compose stack with the new commit-tagged image.
10. Wait for `/_liveness` and `/_readiness`.
11. Run authenticated WebSocket, message, media, and agent-launch smoke tests.
12. Mark the deployment successful and retain the previous local image.

Rollback restores the previous commit-tagged local image and Compose revision
without rewriting Git history. The checkout may remain at the newer `main`
commit while the stack runs the previous image until a corrective commit is
merged. Schema
changes are classified before release:

- `additive`: creates nullable columns, tables, indexes, or compatible data and
  permits normal image rollback.
- `expand-contract`: ships the expand phase first, keeps old and new
  application versions compatible for one release, and delays the contract
  phase until rollback is no longer required.
- `destructive`: removes or irreversibly rewrites data and requires an approved
  maintenance window, verified backup, isolated restore test, and explicit
  database rollback procedure.

Automated deployment accepts only `additive` or the expand phase of
`expand-contract` migrations. A destructive migration requires manual release
approval and cannot claim the normal fifteen-minute image rollback objective.

## Backup and Restore

The backup set contains:

- PostgreSQL logical backup.
- Stable relay private key and HMAC secret.
- Compose environment and non-secret configuration.
- Git persistent data if the deployed version still treats it as durable.
- Object-storage versioning or a separate object backup policy.

PostgreSQL uses continuous WAL archiving to encrypted off-host storage plus a
daily base backup. The production objectives are an RPO of at most one hour and
a disaster RTO of at most four hours. Retention is seven daily, four weekly,
and twelve monthly restore points. Backup success and WAL freshness are
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

The initial service objectives are:

| Measure | Pilot objective |
|---|---|
| Pilot size | 20 employees |
| Validated capacity | 100 concurrent WebSocket clients |
| Message delivery latency | p95 below 500 ms on the company network |
| Monthly availability | 99.5%, excluding announced maintenance |
| Normal application rollback | 15 minutes |
| Data RPO | 1 hour |
| Disaster RTO | 4 hours |
| Model catalog response | 5 seconds on a healthy local Gateway |

Capacity testing runs at 100 concurrent clients, five times the 20-person pilot
size, before expanding beyond the pilot ring. The single-node availability
objective explicitly accepts that the Mac Studio, office power, office network,
and tunnel are common failure domains.

## Testing Strategy

Git and release tests verify:

- Protected-branch and tag rules.
- Owner-initiated upstream merge against `main` and forward-merge into
  `develop`.
- Sipher desktop tag parsing and artifact naming.
- Reproducible ARM64 relay build from a recorded `main` commit.
- Commit-tagged deployment and rollback.
- Hotfix forward-merge into `develop`.
- Git fetch or local build failure leaves the running deployment
  unchanged.

AI Gateway tests verify:

- Provider alias resolution.
- Profile status parsing and timeout behavior.
- macOS Keychain and file-fallback credential resolution without logging keys.
- Base URL normalization.
- Detailed model response parsing.
- Authorization and model-detail request headers.
- Loading, offline, refresh, removed-model, and authentication UI states.
- A complete agent turn through a mock OpenAI-compatible Gateway endpoint.
- The packaged signed Desktop includes `buzz-agent`, the production relay URL,
  and the Sipher updater identity.

Mac Studio validation verifies:

- Automatic stack recovery after a planned authenticated restart.
- Physical FileVault recovery after an uncontrolled reboot.
- Backup and full restore into an isolated Compose project.
- Tunnel-only ingress.
- Authenticated relay connection, messaging, media, git, and agent launch.
- Rollback to the immediately previous commit-tagged image.

## Rollout

1. Grant the implementation identity GitHub `Write` access and verify branch,
   package, Actions, and pull-request permissions.
2. Create and protect `develop`, protect `main`, configure Sipher artifact
   ownership, and document the owner-initiated upstream sync procedure.
3. Build and sign the Sipher Desktop release pipeline.
4. Implement AI Gateway support through feature PRs into `develop`.
5. Distribute a signed release candidate to the 20-person engineering ring.
6. Remediate Mac Studio storage, memory, credentials, patching, firewall,
   FileVault, UPS, and service-account startup.
7. Provision and validate the external production media bucket.
8. Pull `main`, build the Sipher relay locally, and validate it in an
   ephemeral stack.
9. Perform database backup, point-in-time recovery, and full restore rehearsal.
10. Deploy the recorded `main` commit and run the 100-connection capacity test.
11. Observe the 20-person pilot for two weeks against the service objectives.
12. Expand internal access only if every acceptance criterion remains
    satisfied.

## Acceptance Criteria

- `develop` is the default protected branch and `main` is the protected
  production branch.
- The implementation identity can push branches, create PRs, publish packages,
  and read the private production image from the Mac Studio.
- The owner-initiated sync procedure documents how a sync branch based on
  `main` preserves Sipher changes, passes CI, and is forward-merged into
  `develop` when synchronization is needed.
- A hotfix merged to `main` is forward-merged into `develop` before closure.
- An employee already authenticated with AI Gateway can select it, refresh its
  advertised models, select a model, and complete an agent turn without
  entering another API key.
- No AI Gateway credential is present in Buzz persisted configuration, relay
  traffic, or logs.
- A signed and notarized Sipher Desktop release candidate installs, opens its
  Sipher deep links, locates the packaged `buzz-agent`, reaches the production
  relay, and checks the Sipher updater endpoint.
- The Mac Studio fast-forwards a clean production checkout to `origin/main`,
  records the commit SHA, builds the relay locally, and passes all health and
  smoke checks.
- The Mac Studio passes planned authenticated restart and physical FileVault
  recovery tests.
- Host storage and memory meet the 500 GB and 8 GB headroom gates throughout
  the pilot.
- The external media bucket passes encryption, versioning, authorization, and
  restore tests; MinIO is absent from production.
- The previous release can be restored without a database restore.
- Additive and expand-contract migration tests prove compatibility with the
  immediately previous relay image.
- A fresh environment can be recovered to a point no older than one hour in
  no more than four hours from the documented off-host backup.
- The 100-connection test meets p95 message latency below 500 ms.
- The 20-person pilot completes two weeks with at least 99.5% availability and
  without critical disk, memory, tunnel, database, source-build, or backup
  alerts.
