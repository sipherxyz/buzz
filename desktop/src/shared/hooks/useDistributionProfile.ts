import * as React from "react";

import {
  getDistributionProfile,
  OSS_DISTRIBUTION_PROFILE,
  type DistributionProfile,
} from "@/shared/api/tauriDistribution";

export function useDistributionProfile(): DistributionProfile | null {
  const [profile, setProfile] = React.useState<DistributionProfile | null>(
    null,
  );

  React.useEffect(() => {
    let active = true;
    void getDistributionProfile()
      .then((next) => {
        if (active) setProfile(next);
      })
      .catch(() => {
        if (active) setProfile(OSS_DISTRIBUTION_PROFILE);
      });
    return () => {
      active = false;
    };
  }, []);

  return profile;
}
