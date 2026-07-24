# Sipher Self-Hosted Onboarding and AI Gateway Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Sipher desktop distribution that joins `wss://buzz.sipher.gg:8443` without Builderlab and launches `buzz-agent` through each employee's local AI Gateway without Buzz reading gateway credentials.

**Architecture:** A compile-time distribution profile is exposed by Tauri and consumed by onboarding, settings, and agent defaults. The existing community transaction remains the only join path. For AI Gateway, Buzz delegates readiness, model discovery, credential access, and process wrapping to the `ai-gateway` CLI; the desktop never reads its Keychain entries or credential files.

**Tech Stack:** Rust, Tauri 2, React 19, TypeScript, Vitest/Node tests, Playwright, GitHub Actions, shell release-contract tests.

---

## File map

- `desktop/src-tauri/src/commands/distribution.rs`: native source of truth for the read-only OSS/Sipher build profile.
- `desktop/src/shared/api/tauriDistribution.ts`: typed frontend API and cached profile loader.
- `desktop/src/features/communities/sipherBootstrap.ts`: pure decision logic for first-run Sipher auto-join.
- `desktop/src/app/App.tsx`: starts the existing community onboarding transaction automatically when the policy requires it.
- `desktop/src/features/settings/ui/SettingsView.tsx`: hides only Builderlab Hosted Communities for Sipher.
- `desktop/src/features/onboarding/ui/DefaultConfigStep.tsx`: applies non-destructive Sipher agent defaults.
- `desktop/src/features/agents/ui/AgentConfigFields.tsx`: presents runtime as “Agent engine” and provider as “LLM connection”.
- `desktop/src-tauri/src/managed_agents/ai_gateway.rs`: CLI status/model parsing and wrapper launch specification; no credential reads.
- `desktop/src-tauri/src/commands/agent_models.rs`: routes AI Gateway model discovery to the CLI.
- `desktop/src-tauri/src/managed_agents/runtime.rs`: launches the selected `buzz-agent` through AI Gateway.
- `desktop/src-tauri/src/managed_agents/readiness.rs`: reports missing, logged-out, ready, and failed gateway states.
- `crates/buzz-agent/src/config.rs`: accepts standard OpenAI variables only when provider is `ai-gateway`.
- `.github/workflows/sipher-desktop-release.yml`: embeds the Sipher policy in Sipher artifacts.
- `scripts/test-sipher-desktop-release-contract.sh`: release and LAN self-registration contract checks.

### Task 1: Native distribution profile and Sipher build contract

**Files:**
- Create: `desktop/src-tauri/src/commands/distribution.rs`
- Create: `desktop/src/shared/api/tauriDistribution.ts`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src-tauri/build.rs`
- Modify: `desktop/src/shared/api/tauri.ts`
- Modify: `desktop/src/testing/e2eBridge.ts`
- Modify: `.github/workflows/sipher-desktop-release.yml`
- Test: `scripts/test-sipher-desktop-release-contract.sh`
- Test: `desktop/src/shared/api/tauriDistribution.test.mjs`

- [ ] **Step 1: Add failing release-contract and frontend policy tests**

Assert that the Sipher workflow sets `BUZZ_BUILD_DISTRIBUTION: sipher`, and that a pure normalizer maps an unknown/native-missing value to:

```ts
{
  id: "oss",
  autoJoinDefaultRelay: false,
  defaultCommunityName: null,
  hostedCommunitiesEnabled: true,
  preferredAgentRuntime: null,
  preferredLlmProvider: null,
}
```

and maps `sipher` to:

```ts
{
  id: "sipher",
  autoJoinDefaultRelay: true,
  defaultCommunityName: "Sipher",
  hostedCommunitiesEnabled: false,
  preferredAgentRuntime: "buzz-agent",
  preferredLlmProvider: "ai-gateway",
}
```

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
bash scripts/test-sipher-desktop-release-contract.sh
cd desktop && node --test src/shared/api/tauriDistribution.test.mjs
```

Expected: FAIL because the workflow variable and distribution API do not exist.

- [ ] **Step 3: Implement the native profile**

Expose a serializable command result:

```rust
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DistributionProfile {
    pub id: &'static str,
    pub auto_join_default_relay: bool,
    pub default_community_name: Option<&'static str>,
    pub hosted_communities_enabled: bool,
    pub preferred_agent_runtime: Option<&'static str>,
    pub preferred_llm_provider: Option<&'static str>,
}

#[tauri::command]
pub fn get_distribution_profile() -> DistributionProfile {
    profile_for(option_env!("BUZZ_BUILD_DISTRIBUTION"))
}
```

Register `get_distribution_profile` in the Tauri invoke handler. Add the build script rerun directive:

```rust
println!("cargo:rerun-if-env-changed=BUZZ_BUILD_DISTRIBUTION");
```

Add a cached frontend loader using `invoke("get_distribution_profile")` and an E2E mock response. The normalizer must fail closed to the OSS profile for unsupported IDs.

- [ ] **Step 4: Embed the Sipher build value**

Set the workflow-level build environment:

```yaml
BUZZ_BUILD_DISTRIBUTION: sipher
```

Extend the shell contract test to assert this value and retain the existing relay URL/provider/model assertions.

- [ ] **Step 5: Run focused tests**

Run:

```bash
bash scripts/test-sipher-desktop-release-contract.sh
cd desktop && node --test src/shared/api/tauriDistribution.test.mjs
cargo test --manifest-path desktop/src-tauri/Cargo.toml distribution
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/sipher-desktop-release.yml scripts/test-sipher-desktop-release-contract.sh desktop/src-tauri/build.rs desktop/src-tauri/src/commands desktop/src-tauri/src/lib.rs desktop/src/shared/api desktop/src/testing/e2eBridge.ts
git commit -m "feat: add Sipher desktop distribution profile"
```

### Task 2: First-run Sipher community auto-join

**Files:**
- Create: `desktop/src/features/communities/sipherBootstrap.ts`
- Test: `desktop/src/features/communities/sipherBootstrap.test.mjs`
- Modify: `desktop/src/app/App.tsx`
- Modify: `desktop/tests/e2e/onboarding.spec.ts`

- [ ] **Step 1: Add failing bootstrap decision tests**

Cover this decision table:

```ts
shouldAutoJoinSipher({
  profile,
  communities: [],
  onboardingTransaction: null,
}) === true;
```

It must return `false` for OSS, any saved community, or an existing onboarding transaction. Add an E2E case that supplies the Sipher distribution, completes machine setup, and expects the profile step without rendering `welcome-create-community`.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
cd desktop
node --test src/features/communities/sipherBootstrap.test.mjs
pnpm exec playwright test tests/e2e/onboarding.spec.ts --project=smoke
```

Expected: unit test fails because the helper is absent; E2E reaches the generic first-community chooser.

- [ ] **Step 3: Implement one-shot auto-join through the existing transaction**

In `CommunityApp`, load the distribution profile and, only when the pure helper permits it, call:

```ts
communityOnboarding.start({
  source: "first-community",
  relayUrl: defaultRelayUrl,
  communityName: profile.defaultCommunityName ?? "Sipher",
});
```

Use a ref keyed by distribution and relay URL to prevent duplicate starts while React rerenders. Render the existing loading gate until the transaction owns the UI. Do not create a second community persistence path.

- [ ] **Step 4: Add recovery behavior**

Keep the existing onboarding error/retry behavior. On Sipher connection failure, expose the existing cancel/add-community route as “Connect another relay”; an explicit relay membership denial must keep the native error text visible rather than marking onboarding complete.

- [ ] **Step 5: Run focused tests**

Run the commands from Step 2. Expected: PASS, and the Sipher E2E contains no Builderlab modal.

- [ ] **Step 6: Commit**

```bash
git add desktop/src/app/App.tsx desktop/src/features/communities/sipherBootstrap.ts desktop/src/features/communities/sipherBootstrap.test.mjs desktop/tests/e2e/onboarding.spec.ts
git commit -m "feat: auto-join Sipher community on first run"
```

### Task 3: Hide hosted provisioning and apply non-destructive agent defaults

**Files:**
- Create: `desktop/src/features/settings/settingsVisibility.ts`
- Test: `desktop/src/features/settings/settingsVisibility.test.mjs`
- Create: `desktop/src/features/onboarding/sipherAgentDefaults.ts`
- Test: `desktop/src/features/onboarding/sipherAgentDefaults.test.mjs`
- Modify: `desktop/src/features/settings/ui/SettingsView.tsx`
- Modify: `desktop/src/features/onboarding/ui/DefaultConfigStep.tsx`
- Modify: `desktop/src/features/agents/ui/AgentConfigFields.tsx`
- Modify: `desktop/src/features/agents/ui/agentConfigOptions.test.mjs`

- [ ] **Step 1: Add failing visibility and merge tests**

Test that `hosted-communities` is filtered only when `hostedCommunitiesEnabled` is false. Test a pure defaults merge with these invariants:

```ts
applyDistributionAgentDefaults({}, sipherProfile)
// => { command: "buzz-agent", provider: "ai-gateway" }

applyDistributionAgentDefaults(
  { command: "claude-code", provider: "anthropic" },
  sipherProfile,
)
// preserves both existing values
```

Also assert the field labels are “Agent engine” and “LLM connection”.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
cd desktop
node --test src/features/settings/settingsVisibility.test.mjs
node --test src/features/onboarding/sipherAgentDefaults.test.mjs
node --test src/features/agents/ui/agentConfigOptions.test.mjs
```

Expected: FAIL because the policy helpers and new labels do not exist.

- [ ] **Step 3: Implement distribution-aware settings visibility**

Load the cached distribution profile in `SettingsView` and pass its flag into the pure filter. Preserve all feature-gate and membership-role checks. If the current section becomes hidden, keep the existing redirect to the first visible section.

- [ ] **Step 4: Implement Sipher defaults without overwrites**

When `DefaultConfigStep` loads global config, fill only absent runtime/provider fields from the distribution profile. Keep model empty and required until live gateway discovery succeeds. Preserve the runtime catalog and current provider option descriptors as the source of capabilities.

- [ ] **Step 5: Clarify UI semantics**

Change the user-facing labels to:

```text
Agent engine
LLM connection
Model
```

Retain alternate runtimes/providers in the existing selection controls so advanced users can switch away from the defaults.

- [ ] **Step 6: Run focused tests and commit**

Run all commands from Step 2; expected PASS.

```bash
git add desktop/src/features/settings desktop/src/features/onboarding desktop/src/features/agents/ui
git commit -m "feat: tailor Sipher onboarding and settings"
```

### Task 4: Replace desktop credential access with AI Gateway CLI contracts

**Files:**
- Modify: `desktop/src-tauri/src/managed_agents/ai_gateway.rs`
- Test: `desktop/src-tauri/src/managed_agents/ai_gateway.rs`
- Modify: `desktop/src-tauri/Cargo.toml`

- [ ] **Step 1: Replace credential-resolution tests with failing CLI parsing tests**

Add tests for:

```rust
parse_status_output(valid_ready_json) == AiGatewayStatus::Ready
parse_status_output(valid_logged_out_json) == AiGatewayStatus::LoggedOut
parse_models_output("OpenAI\n  gpt-5.1\nAnthropic\n  claude-sonnet-4-5")
```

The models parser must return unique model IDs in stable output order, reject empty/malformed output, and never return credentials. Add launch-spec tests for both POSIX absolute paths and Windows paths ending in `ai-gateway.exe` and `buzz-agent.exe`, with no shell string construction. The POSIX case expects:

```rust
AiGatewayLaunchSpec {
    command: "/resolved/ai-gateway",
    args: vec![
        "--prod", "run", "/resolved/buzz-agent",
        "--expose", "openai",
    ],
}
```

- [ ] **Step 2: Verify the new tests fail**

Run:

```bash
just _ensure-sidecar-stubs
cargo test --manifest-path desktop/src-tauri/Cargo.toml ai_gateway
```

Expected: FAIL because status/models/launch parsing is missing and old tests still expect keyring/file resolution.

- [ ] **Step 3: Remove credential ownership from Buzz**

Delete `key_from_keyring`, `key_from_file`, `resolve_key`, `resolve_ai_gateway_config`, `apply_ai_gateway_env`, and their credential dependencies. Keep executable/profile selection. Implement bounded command runners for:

```text
ai-gateway --prod status --offline
ai-gateway --prod models
```

Return typed errors containing the subcommand, exit status, and sanitized stderr; never include environment values.

- [ ] **Step 4: Implement launch specification**

Build the wrapper arguments from resolved executable paths and preserve original `buzz-agent` arguments before the final `--expose openai` flag in the order accepted by the CLI. Include the selected profile in the launch spec.

- [ ] **Step 5: Run focused tests and commit**

Run the Step 2 command; expected PASS and no test references Keychain, `cliproxy`, or gateway credential files.

```bash
git add desktop/src-tauri/src/managed_agents/ai_gateway.rs desktop/src-tauri/Cargo.toml
git commit -m "refactor: delegate AI Gateway credentials to CLI"
```

### Task 5: Route live model discovery through `ai-gateway models`

**Files:**
- Modify: `desktop/src-tauri/src/commands/agent_models.rs`
- Modify: `desktop/src-tauri/src/commands/agent_models_tests.rs`

- [ ] **Step 1: Add failing command-routing tests**

Inject a fake command runner and assert:

```text
provider=ai-gateway -> ai-gateway --prod models
provider=openai-compatible -> existing HTTP /models discovery
provider=anthropic -> existing ACP model discovery
```

Assert AI Gateway failures produce `AgentModelsResponse.error`, an empty model list is an error, duplicate IDs are removed, and a saved but absent model is not replaced automatically.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
just _ensure-sidecar-stubs
cargo test --manifest-path desktop/src-tauri/Cargo.toml agent_models
```

Expected: FAIL because AI Gateway still calls `apply_ai_gateway_env` and HTTP discovery.

- [ ] **Step 3: Implement the provider-specific CLI branch**

Before the generic OpenAI-compatible HTTP branch:

```rust
if provider == AI_GATEWAY_PROVIDER_ID {
    return discover_ai_gateway_models(profile).await;
}
```

Map parsed IDs into the existing model response contract. Do not add a fallback catalog or read a gateway API key.

- [ ] **Step 4: Run focused tests and commit**

Run the Step 2 command; expected PASS.

```bash
git add desktop/src-tauri/src/commands/agent_models.rs desktop/src-tauri/src/commands/agent_models_tests.rs
git commit -m "feat: discover models through AI Gateway CLI"
```

### Task 6: Wrap `buzz-agent` launches and accept gateway-injected variables

**Files:**
- Modify: `desktop/src-tauri/src/managed_agents/runtime.rs`
- Modify: `desktop/src-tauri/src/managed_agents/spawn_hash.rs`
- Modify: `desktop/src-tauri/src/managed_agents/spawn_hash/tests.rs`
- Modify: `crates/buzz-agent/src/config.rs`

- [ ] **Step 1: Add failing launch and configuration tests**

Test that only `command=buzz-agent` plus `provider=ai-gateway` changes the ACP command to AI Gateway. Assert the original absolute agent path and arguments remain in the wrapper args, other combinations are unchanged, and `prod` versus `dev` changes the spawn hash.

In `buzz-agent`, add tests showing:

```text
BUZZ_AGENT_PROVIDER=ai-gateway
OPENAI_API_KEY=managed-by-gateway
OPENAI_BASE_URL=http://127.0.0.1:<port>/v1
```

builds an OpenAI-compatible client, while `BUZZ_AGENT_PROVIDER=openai-compatible` still requires `OPENAI_COMPAT_API_KEY` and `OPENAI_COMPAT_BASE_URL`.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
just _ensure-sidecar-stubs
cargo test --manifest-path desktop/src-tauri/Cargo.toml managed_agents
cargo test -p buzz-agent config
```

Expected: FAIL because runtime injects credentials directly and `buzz-agent` ignores the standard variables for `ai-gateway`.

- [ ] **Step 3: Apply the launch wrapper**

Resolve effective provider/model before constructing ACP environment. For the exact `buzz-agent`/`ai-gateway` pair, set:

```text
BUZZ_ACP_AGENT_COMMAND=<absolute ai-gateway>
BUZZ_ACP_AGENT_ARGS=--prod,run,<absolute buzz-agent>,<original args>,--expose,openai
```

Remove the direct `OPENAI_COMPAT_API_KEY` and `OPENAI_COMPAT_BASE_URL` injection block. Preserve process-group ownership so stopping the managed agent terminates the gateway wrapper and child.

- [ ] **Step 4: Update spawn identity and agent configuration**

Hash the effective wrapper command, wrapper args, and profile. In `buzz-agent`, select standard OpenAI variables only inside the explicit `ai-gateway` provider branch; keep current OpenAI-compatible behavior unchanged.

- [ ] **Step 5: Run focused tests and commit**

Run the Step 2 commands; expected PASS.

```bash
git add desktop/src-tauri/src/managed_agents/runtime.rs desktop/src-tauri/src/managed_agents/spawn_hash.rs desktop/src-tauri/src/managed_agents/spawn_hash/tests.rs crates/buzz-agent/src/config.rs
git commit -m "feat: launch Buzz Agent through AI Gateway"
```

### Task 7: AI Gateway readiness and actionable errors

**Files:**
- Modify: `desktop/src-tauri/src/managed_agents/readiness.rs`
- Modify: `desktop/src-tauri/src/managed_agents/readiness/cli_probe.rs`
- Modify: `desktop/src-tauri/src/managed_agents/ai_gateway.rs`
- Test: `desktop/src-tauri/src/managed_agents/readiness.rs`

- [ ] **Step 1: Add failing readiness tests**

Cover four states:

```text
binary missing -> CliMissing with installation guidance
status logged out -> NotInstalled with `ai-gateway login`
status ready -> Available
unsupported/failing response -> AdapterMissing with retryable detail
```

Verify this extra probe applies only to `buzz-agent` plus provider `ai-gateway`.

- [ ] **Step 2: Verify the tests fail**

Run:

```bash
just _ensure-sidecar-stubs
cargo test --manifest-path desktop/src-tauri/Cargo.toml readiness
```

Expected: FAIL because current readiness does not probe the gateway CLI.

- [ ] **Step 3: Integrate the status probe**

Reuse the typed result from `ai_gateway.rs`, map it into the existing readiness response, and keep the UI's existing Retry mechanism. Error messages must name the failed operation but must not include credential paths or values.

- [ ] **Step 4: Run focused tests and commit**

Run the Step 2 command; expected PASS.

```bash
git add desktop/src-tauri/src/managed_agents/readiness.rs desktop/src-tauri/src/managed_agents/readiness/cli_probe.rs desktop/src-tauri/src/managed_agents/ai_gateway.rs
git commit -m "feat: report AI Gateway readiness"
```

### Task 8: Deployment contract, complete verification, and unsigned macOS artifact

**Files:**
- Modify: `deploy/macstudio/preflight.sh`
- Modify: `deploy/macstudio/README.md`
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-07-24-sipher-selfhost-onboarding-ai-gateway-design.md`

- [ ] **Step 1: Add failing preflight contract tests**

Extend the deployment script test to assert the Sipher LAN profile requires:

```env
BUZZ_REQUIRE_AUTH_TOKEN=false
BUZZ_REQUIRE_RELAY_MEMBERSHIP=false
```

and reports the LAN trust boundary as a warning/accepted policy, not a generic production-auth failure. Retain FileVault and firewall as warnings, matching the approved Mac Studio policy.

- [ ] **Step 2: Verify the deployment test fails**

Run the existing Mac Studio preflight test or, if the script is self-testing:

```bash
bash -n deploy/macstudio/preflight.sh
rg -n 'BUZZ_REQUIRE_AUTH_TOKEN|BUZZ_REQUIRE_RELAY_MEMBERSHIP|FileVault|Firewall' deploy/macstudio
```

Expected: contract mismatch is visible before the documentation and checks are updated.

- [ ] **Step 3: Update operator and employee documentation**

Document:

- `buzz.sipher.gg:8443` is the community/relay, not an account service;
- reachability is limited by LAN/VPN/firewall because relay membership is open;
- AI Gateway must be installed and logged in on each employee machine;
- the desktop does not need or store the gateway secret;
- advanced relay/runtime/provider switching remains available;
- Builderlab is not required by a Sipher build.

- [ ] **Step 4: Run all quality gates**

Run:

```bash
. ./bin/activate-hermit
just fix-all
just ci
just test
cd desktop && pnpm exec playwright test tests/e2e/onboarding.spec.ts --project=smoke
```

Expected: formatting, lint, Rust/desktop tests, integration tests, and onboarding E2E pass. If integration infrastructure is unavailable, record the exact missing service and retain all runnable green gates.

- [ ] **Step 5: Build the unsigned macOS test artifact**

Run the repository's unsigned Sipher desktop build path with:

```bash
BUZZ_BUILD_DISTRIBUTION=sipher
BUZZ_DEFAULT_RELAY_URL=wss://buzz.sipher.gg:8443
```

Inspect the resulting `.dmg`, install it on a clean macOS account, and smoke-test:

1. no Builderlab login or Create Community modal;
2. local identity and Sipher profile onboarding;
3. model list comes from `ai-gateway --prod models`;
4. starting an agent produces the expected process chain;
5. no Buzz-to-`ai-gateway` Keychain authorization modal;
6. custom relay switching remains available.

- [ ] **Step 6: Update the design status and commit**

Mark the spec implemented with verification evidence.

```bash
git add deploy/macstudio README.md docs/superpowers/specs
git commit -m "docs: finalize Sipher desktop deployment"
```

- [ ] **Step 7: Review branch for merge**

Run:

```bash
git status --short
git log --oneline origin/main..HEAD
git diff --check origin/main...HEAD
```

Expected: clean worktree, focused commits, and no whitespace errors.
