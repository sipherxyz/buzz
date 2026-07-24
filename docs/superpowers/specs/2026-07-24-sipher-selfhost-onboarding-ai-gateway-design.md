# Sipher Self-Hosted Onboarding and AI Gateway Design

**Date:** 2026-07-24  
**Status:** Approved for implementation planning  
**Target repository:** `sipherxyz/buzz`  
**Target distribution:** Sipher-managed desktop builds

## Objective

Make the Sipher desktop build usable without a Builderlab account:

1. A first-run client creates a local Buzz identity.
2. The client joins the self-hosted Sipher workspace at
   `wss://buzz.sipher.gg:8443`.
3. The client creates its profile and enters the app without an invite because
   the relay permits self-registration from the company LAN.
4. Managed agents use the employee's locally installed AI Gateway by default.
5. Advanced users can still add or switch to other relays, harnesses, and LLM
   providers.

The OSS build must retain its current generic first-community and Builderlab
hosted-community flows.

## Product Semantics

### Identity, account, and community

Buzz does not create a username/password account on the relay. The desktop
generates a Nostr keypair and stores the private key in the platform secure
store. The public key is the employee's Buzz identity.

For the default single-relay deployment, the relay URL is the community
boundary. Therefore:

```text
wss://buzz.sipher.gg:8443 = the Sipher community
```

There is no separate "create community" API call during Sipher onboarding.
The relay process bootstraps the host-derived deployment community. The
desktop creates only its local `Community` record and then publishes the
employee profile to the relay.

Builderlab provisioning remains relevant only to the OSS hosted-community
path, where a control plane must allocate a new
`*.communities.buzz.xyz` hostname.

### Self-registration security boundary

The Sipher relay intentionally runs with:

```env
BUZZ_REQUIRE_AUTH_TOKEN=false
BUZZ_REQUIRE_RELAY_MEMBERSHIP=false
```

Anyone who can reach the relay can create an identity and publish events.
The LAN, VPN, firewall, and DNS exposure are therefore the access perimeter.
The deployment preflight must verify the two settings above for the
self-registration release profile and document the accepted LAN trust model.

If either relay policy changes later, the desktop must fail visibly instead of
pretending onboarding succeeded.

## Chosen Approach

Use an explicit build-time distribution policy.

The Sipher release workflow embeds a `sipher` distribution identifier. The
native layer exposes a read-only distribution profile to the frontend. Product
behavior is selected from that profile, never by checking whether a URL happens
to contain `sipher.gg`.

The profile contains:

- distribution ID;
- default community name;
- whether first-run auto-joins the compiled default relay;
- whether Builderlab hosted-community management is visible;
- preferred bundled agent runtime;
- preferred LLM provider.

Expected profiles:

| Policy | OSS build | Sipher build |
|---|---|---|
| Distribution ID | `oss` | `sipher` |
| Auto-join default relay | No | Yes |
| Default community name | Derived/user-provided | `Sipher` |
| Hosted communities UI | Visible | Hidden |
| Preferred agent runtime | Existing behavior | `buzz-agent` |
| Preferred LLM provider | Existing behavior | `ai-gateway` |
| Add arbitrary relay | Yes | Yes |
| Advanced harness/provider choice | Yes | Yes |

This keeps the fork changes isolated, reviewable, and resilient to future
upstream syncs.

## First-Run Onboarding

### New Sipher installation

The first-run sequence is:

1. Native startup loads or creates the local Nostr identity using the existing
   secure identity store.
2. The frontend loads the embedded distribution profile.
3. When the profile is `sipher` and there are no saved communities, onboarding
   starts a normal community-onboarding transaction against the compiled
   default relay with the display name `Sipher`.
4. The relay connection is probed before the local community is finalized.
5. The app checks relay membership behavior. An open relay advances; an
   explicit membership denial is shown as a deployment-policy error.
6. The employee completes the existing profile fields.
7. The app configures AI Gateway defaults and completes onboarding.

The first-community chooser and Builderlab sign-in modal do not appear on this
path.

### Idempotency and recovery

- Auto-join runs only when the saved community list is empty.
- Existing users are never moved to `buzz.sipher.gg` automatically.
- Existing global agent defaults are never overwritten.
- An interrupted community transaction resumes through the existing
  transaction mechanism and does not create duplicates.
- The community record is finalized only after the relay is reachable.
- Relay failures provide `Retry` and an advanced `Connect another relay`
  action.
- A membership denial explains that the Sipher relay is not configured for
  LAN self-registration and directs the user to the operator.

### Advanced community management

After onboarding, Sipher users retain the normal community switcher and custom
relay connection flow. They can add an invite URL or arbitrary `ws://`/`wss://`
relay.

The Builderlab-specific Hosted Communities settings panel is hidden only in
the Sipher distribution. It is not deleted, and it remains available in OSS
builds.

## AI Gateway User Experience

### Configuration model

The UI must distinguish three concepts:

- **Agent engine:** the ACP-compatible harness/runtime, defaulting to
  `Buzz Agent`.
- **LLM connection:** the LLM provider/transport, defaulting to `AI Gateway`.
- **Model:** a model ID reported by the local AI Gateway.

AI Gateway is a provider/launcher integration, not an ACP harness.

On the main onboarding surface, Sipher shows the AI Gateway connection and
model fields. Harness selection and non-gateway providers remain available
under Advanced. Existing generic configuration descriptors and the Rust
runtime catalog remain authoritative; the Sipher UI must not introduce a
second hardcoded harness-capability table.

### Readiness states

The desktop invokes the AI Gateway CLI for readiness:

```text
ai-gateway --prod status --offline
```

The UI distinguishes:

- executable not installed;
- installed but not logged in;
- authenticated and ready;
- command failed or returned an unsupported response.

The error state includes the relevant installation or `ai-gateway login`
instruction and a Retry action. Selecting another provider from Advanced
remains possible.

### Model discovery

The desktop invokes the documented CLI command:

```text
ai-gateway --prod models
```

It parses provider headings and model IDs into the existing
`AgentModelsResponse` contract. Model IDs come only from live gateway output;
there is no built-in AI Gateway fallback catalog. Empty, malformed, timed-out,
or failed output is an explicit discovery error.

For a new Sipher configuration, a model selection is required before an agent
can start. Existing selected models are preserved when they remain present in
the returned catalog. The app does not silently choose a different paid model
when a saved model disappears.

The CLI invocation owns secure-store access, so the desktop process never needs
the gateway API key to list models.

## AI Gateway Process Routing

### Root cause being removed

The current integration resolves the AI Gateway base URL and credential inside
the Buzz desktop process. On macOS it opens the `ai-gateway` or `cliproxy`
Keychain entry under the Buzz application identity, producing repeated
authorization prompts.

That design violates credential ownership and must be removed.

### Target launch chain

For `buzz-agent` with provider `ai-gateway`, the managed runtime configures the
ACP harness to spawn AI Gateway as the agent command:

```text
Buzz Desktop
  -> buzz-acp
  -> ai-gateway --prod run <absolute bundled buzz-agent path> --expose openai
  -> buzz-agent
  -> AI Gateway HTTP endpoint
  -> selected LLM
```

The wrapper preserves the original `buzz-agent` arguments and stdio. It is
applied only to the `buzz-agent` plus `ai-gateway` combination. Other harnesses
and providers retain their current spawn paths.

AI Gateway injects standard OpenAI-compatible variables into `buzz-agent`:

```text
OPENAI_API_KEY
OPENAI_BASE_URL
```

When `BUZZ_AGENT_PROVIDER=ai-gateway`, `buzz-agent` accepts those standard
variables as its credential and base URL source. The existing
`OPENAI_COMPAT_*` variables remain supported for current OpenAI-compatible
configurations.

The desktop must not:

- read `ai-gateway` or `cliproxy` Keychain entries;
- read gateway secret files;
- persist gateway credentials in global, persona, or agent records;
- inject a gateway API key into the long-lived `buzz-acp` process;
- log gateway credentials or environment values.

### Lifecycle

The existing process-group lifecycle remains authoritative. Stopping or
restarting the managed agent must terminate `buzz-acp`, its `ai-gateway`
launcher, and `buzz-agent`. Wrapper command and arguments participate in the
spawn hash so a provider/profile change causes the expected restart.

The implementation must resolve absolute executable paths for packaged macOS
and Windows builds. Windows uses `ai-gateway.exe` and `buzz-agent.exe`; no
shell-specific command construction is allowed.

## Error Handling

| Failure | User-facing behavior |
|---|---|
| `buzz.sipher.gg` unreachable | Keep onboarding transaction, show Retry and advanced relay option |
| Relay unexpectedly membership-gated | Explain Sipher deployment-policy mismatch; do not claim success |
| Identity secure store locked | Reuse existing keyring recovery screen |
| AI Gateway executable missing | Show install instruction and Retry |
| AI Gateway not authenticated | Show `ai-gateway login` instruction and Retry |
| AI Gateway model command fails | Show live discovery failure; do not show fallback models |
| Saved model absent from catalog | Preserve value, mark unavailable, require explicit replacement before agent start |
| Wrapped agent exits | Surface the existing managed-agent runtime error with credential values redacted |

## Build and Deployment Contract

The Sipher desktop release workflow:

- embeds `BUZZ_BUILD_DISTRIBUTION=sipher`;
- continues to embed `BUZZ_RELAY_URL=wss://buzz.sipher.gg:8443`;
- continues to embed `BUZZ_RELAY_HTTP=https://buzz.sipher.gg:8443`;
- packages `buzz-agent`, `buzz-acp`, and the other existing sidecars;
- does not package AI Gateway, because every employee installs and authenticates
  the company-managed AI Gateway separately.

Mac Studio deployment preflight validates that the relay's authentication and
membership flags match LAN self-registration. DNS, TLS, PostgreSQL, Redis, S3,
and relay health remain existing deployment responsibilities.

## Compatibility and Migration

- No data migration is required.
- Existing desktop identities remain unchanged.
- Existing community lists and active-community choices remain unchanged.
- Existing global, persona, and per-agent model/provider values remain
  unchanged.
- The Sipher defaults apply only when the corresponding first-run state is
  absent.
- OSS builds retain Builderlab onboarding and hosted-community settings.
- Removing the Sipher distribution flag restores OSS behavior without source
  edits.
- The implementation remains cross-platform; macOS is the first release gate,
  and Windows path/process behavior is covered before a Windows artifact is
  published.

## Verification Strategy

### Unit and contract tests

- Distribution profile defaults for OSS and Sipher.
- Release workflow embeds the explicit Sipher distribution flag and relay URLs.
- First-community auto-join is idempotent and does not affect existing users.
- Builderlab surfaces are visible for OSS and hidden for Sipher.
- AI Gateway command construction uses argument arrays, absolute paths, and the
  correct profile on macOS and Windows.
- AI Gateway status and model output parsing covers ready, missing, logged-out,
  empty, malformed, and command-failure states.
- `buzz-agent` accepts standard OpenAI variables only for the intended
  OpenAI-compatible provider path.
- No desktop code path calls gateway keyring/file credential readers.
- Spawn hashing includes the effective gateway wrapper.

### Desktop E2E tests

- A clean Sipher first run creates the `Sipher` community at the compiled relay
  without rendering Builderlab.
- Profile onboarding reaches the normal application.
- Relay failure and unexpected membership denial render recoverable errors.
- Existing community data bypasses auto-join.
- Advanced settings can add and switch to another relay.
- AI Gateway is the default LLM connection with Buzz Agent as the engine.
- Advanced controls can select a different harness/provider.
- Live model discovery populates the model picker from a fake AI Gateway
  executable.

### Manual macOS release smoke test

1. Install and authenticate AI Gateway.
2. Install a clean unsigned or signed Sipher DMG.
3. Confirm no Builderlab page appears.
4. Confirm the local identity and `Sipher` community are created.
5. Select a model returned by AI Gateway.
6. Create an agent and send a prompt.
7. Confirm AI Gateway usage/logs record the request.
8. Confirm no dialog says Buzz wants access to the `ai-gateway` or `cliproxy`
   Keychain item.
9. Add a second relay from Advanced settings and switch between communities.

### Windows release gate

- Repeat process construction and model discovery tests with `.exe` paths.
- Install the unsigned test artifact on Windows.
- Confirm AI Gateway launches `buzz-agent.exe` without a console-window leak
  and requests appear in the gateway usage log.

## Non-Goals

- Hosting multiple Sipher communities behind different subdomains.
- Adding username/password accounts to Buzz relay.
- Packaging or auto-installing AI Gateway inside Buzz.
- Changing AI Gateway's secure-store implementation.
- Removing Builderlab support from the OSS application.
- Restricting the Sipher desktop permanently to one relay.
- Removing existing harnesses or direct-provider integrations.
