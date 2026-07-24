import assert from "node:assert/strict";
import test from "node:test";

import { applyDistributionAgentDefaults } from "./sipherAgentDefaults.ts";

const sipherProfile = {
  preferredAgentRuntime: "buzz-agent",
  preferredLlmProvider: "ai-gateway",
};

test("Sipher fills absent runtime and LLM connection defaults", () => {
  assert.deepEqual(
    applyDistributionAgentDefaults(
      {
        env_vars: {},
        model: null,
        preferred_runtime: null,
        provider: null,
      },
      sipherProfile,
    ),
    {
      env_vars: {},
      model: null,
      preferred_runtime: "buzz-agent",
      provider: "ai-gateway",
    },
  );
});

test("Sipher never overwrites existing agent defaults", () => {
  const existing = {
    env_vars: { KEEP: "yes" },
    model: "claude-sonnet",
    preferred_runtime: "claude",
    provider: "anthropic",
  };
  assert.deepEqual(
    applyDistributionAgentDefaults(existing, sipherProfile),
    existing,
  );
});
