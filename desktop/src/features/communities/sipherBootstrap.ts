import type { DistributionProfile } from "@/shared/api/tauriDistribution";

export function shouldAutoJoinSipher({
  profile,
  communityCount,
  hasOnboardingTransaction,
}: {
  profile: DistributionProfile;
  communityCount: number;
  hasOnboardingTransaction: boolean;
}): boolean {
  return (
    profile.id === "sipher" &&
    profile.autoJoinDefaultRelay &&
    communityCount === 0 &&
    !hasOnboardingTransaction
  );
}
