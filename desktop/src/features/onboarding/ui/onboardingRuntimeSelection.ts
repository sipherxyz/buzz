import type { AcpRuntimeCatalogEntry } from "@/shared/api/types";

export const ONBOARDING_RUNTIME_ORDER = [
  "claude",
  "codex",
  "goose",
  "buzz-agent",
];

const VISIBLE_ONBOARDING_RUNTIME_IDS = new Set<string>(
  ONBOARDING_RUNTIME_ORDER,
);

export function runtimeIsVisibleInOnboarding(
  runtimeId: string,
  preferredRuntimeId?: string | null,
) {
  return (
    runtimeId === preferredRuntimeId ||
    VISIBLE_ONBOARDING_RUNTIME_IDS.has(runtimeId)
  );
}

export function runtimeIsReadyForOnboarding(runtime: AcpRuntimeCatalogEntry) {
  return (
    runtime.availability === "available" &&
    (runtime.authStatus.status === "logged_in" ||
      runtime.authStatus.status === "not_applicable")
  );
}

export function getVisibleOnboardingRuntimes(
  runtimes: readonly AcpRuntimeCatalogEntry[],
  preferredRuntimeId?: string | null,
) {
  const runtimeOrder = preferredRuntimeId
    ? [
        preferredRuntimeId,
        ...ONBOARDING_RUNTIME_ORDER.filter((id) => id !== preferredRuntimeId),
      ]
    : ONBOARDING_RUNTIME_ORDER;
  return runtimes
    .filter((runtime) =>
      runtimeIsVisibleInOnboarding(runtime.id, preferredRuntimeId),
    )
    .sort(
      (left, right) =>
        runtimeOrder.indexOf(left.id) - runtimeOrder.indexOf(right.id),
    );
}

export function getReadyOnboardingRuntimes(
  runtimes: readonly AcpRuntimeCatalogEntry[],
  preferredRuntimeId?: string | null,
) {
  return getVisibleOnboardingRuntimes(runtimes, preferredRuntimeId).filter(
    runtimeIsReadyForOnboarding,
  );
}
