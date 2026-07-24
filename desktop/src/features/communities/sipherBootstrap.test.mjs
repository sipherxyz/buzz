import assert from "node:assert/strict";
import test from "node:test";

import { shouldAutoJoinSipher } from "./sipherBootstrap.ts";

const sipherProfile = {
  id: "sipher",
  autoJoinDefaultRelay: true,
  defaultCommunityName: "Sipher",
  hostedCommunitiesEnabled: false,
  preferredAgentRuntime: "buzz-agent",
  preferredLlmProvider: "ai-gateway",
};

test("a clean Sipher install auto-joins the compiled relay", () => {
  assert.equal(
    shouldAutoJoinSipher({
      profile: sipherProfile,
      communityCount: 0,
      hasOnboardingTransaction: false,
    }),
    true,
  );
});

test("Sipher auto-join never replaces existing or interrupted setup", () => {
  assert.equal(
    shouldAutoJoinSipher({
      profile: sipherProfile,
      communityCount: 1,
      hasOnboardingTransaction: false,
    }),
    false,
  );
  assert.equal(
    shouldAutoJoinSipher({
      profile: sipherProfile,
      communityCount: 0,
      hasOnboardingTransaction: true,
    }),
    false,
  );
});

test("OSS builds keep generic first-community onboarding", () => {
  assert.equal(
    shouldAutoJoinSipher({
      profile: { ...sipherProfile, id: "oss", autoJoinDefaultRelay: false },
      communityCount: 0,
      hasOnboardingTransaction: false,
    }),
    false,
  );
});
