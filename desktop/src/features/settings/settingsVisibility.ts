export function distributionAllowsSettingsSection(
  section: string,
  profile: { hostedCommunitiesEnabled: boolean },
): boolean {
  return section !== "hosted-communities" || profile.hostedCommunitiesEnabled;
}
