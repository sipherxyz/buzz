import type { GlobalAgentConfig } from "@/shared/api/types";

export function applyDistributionAgentDefaults(
  config: GlobalAgentConfig,
  profile: {
    preferredAgentRuntime: string | null;
    preferredLlmProvider: string | null;
  },
): GlobalAgentConfig {
  const preferredRuntime = config.preferred_runtime?.trim()
    ? config.preferred_runtime
    : profile.preferredAgentRuntime;
  const provider = config.provider?.trim()
    ? config.provider
    : profile.preferredLlmProvider;

  if (
    preferredRuntime === config.preferred_runtime &&
    provider === config.provider
  ) {
    return config;
  }
  return {
    ...config,
    preferred_runtime: preferredRuntime,
    provider,
  };
}
