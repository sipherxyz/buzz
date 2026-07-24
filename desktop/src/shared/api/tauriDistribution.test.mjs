import assert from "node:assert/strict";
import test from "node:test";

import {
  OSS_DISTRIBUTION_PROFILE,
  normalizeDistributionProfile,
} from "./tauriDistribution.ts";

test("unknown distribution values fail closed to the OSS profile", () => {
  assert.deepEqual(normalizeDistributionProfile(undefined), {
    ...OSS_DISTRIBUTION_PROFILE,
  });
  assert.deepEqual(normalizeDistributionProfile({ id: "unexpected" }), {
    ...OSS_DISTRIBUTION_PROFILE,
  });
});

test("Sipher distribution enables internal onboarding defaults", () => {
  assert.deepEqual(normalizeDistributionProfile({ id: "sipher" }), {
    id: "sipher",
    autoJoinDefaultRelay: true,
    defaultCommunityName: "Sipher",
    hostedCommunitiesEnabled: false,
    preferredAgentRuntime: "buzz-agent",
    preferredLlmProvider: "ai-gateway",
  });
});
