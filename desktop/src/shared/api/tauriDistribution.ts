import { invokeTauri } from "@/shared/api/tauri";

export type DistributionProfile = {
  id: "oss" | "sipher";
  autoJoinDefaultRelay: boolean;
  defaultCommunityName: string | null;
  hostedCommunitiesEnabled: boolean;
  preferredAgentRuntime: string | null;
  preferredLlmProvider: string | null;
};

export const OSS_DISTRIBUTION_PROFILE: DistributionProfile = {
  id: "oss",
  autoJoinDefaultRelay: false,
  defaultCommunityName: null,
  hostedCommunitiesEnabled: true,
  preferredAgentRuntime: null,
  preferredLlmProvider: null,
};

export const SIPHER_DISTRIBUTION_PROFILE: DistributionProfile = {
  id: "sipher",
  autoJoinDefaultRelay: true,
  defaultCommunityName: "Sipher",
  hostedCommunitiesEnabled: false,
  preferredAgentRuntime: "buzz-agent",
  preferredLlmProvider: "ai-gateway",
};

export function normalizeDistributionProfile(
  value: unknown,
): DistributionProfile {
  if (
    typeof value === "object" &&
    value !== null &&
    "id" in value &&
    value.id === "sipher"
  ) {
    return { ...SIPHER_DISTRIBUTION_PROFILE };
  }
  return { ...OSS_DISTRIBUTION_PROFILE };
}

let distributionProfilePromise: Promise<DistributionProfile> | null = null;

export function getDistributionProfile(): Promise<DistributionProfile> {
  distributionProfilePromise ??= invokeTauri<unknown>(
    "get_distribution_profile",
  ).then(normalizeDistributionProfile);
  return distributionProfilePromise;
}

export function resetDistributionProfileCache(): void {
  distributionProfilePromise = null;
}
