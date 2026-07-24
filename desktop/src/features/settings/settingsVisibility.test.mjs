import assert from "node:assert/strict";
import test from "node:test";

import { distributionAllowsSettingsSection } from "./settingsVisibility.ts";

test("Sipher hides only Builderlab hosted community settings", () => {
  const profile = { hostedCommunitiesEnabled: false };
  assert.equal(
    distributionAllowsSettingsSection("hosted-communities", profile),
    false,
  );
  assert.equal(
    distributionAllowsSettingsSection("community-members", profile),
    true,
  );
  assert.equal(distributionAllowsSettingsSection("agents", profile), true);
});

test("OSS keeps hosted community settings visible", () => {
  assert.equal(
    distributionAllowsSettingsSection("hosted-communities", {
      hostedCommunitiesEnabled: true,
    }),
    true,
  );
});
