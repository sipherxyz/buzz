#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DistributionProfile {
    pub id: &'static str,
    pub auto_join_default_relay: bool,
    pub default_community_name: Option<&'static str>,
    pub hosted_communities_enabled: bool,
    pub preferred_agent_runtime: Option<&'static str>,
    pub preferred_llm_provider: Option<&'static str>,
}

const OSS_PROFILE: DistributionProfile = DistributionProfile {
    id: "oss",
    auto_join_default_relay: false,
    default_community_name: None,
    hosted_communities_enabled: true,
    preferred_agent_runtime: None,
    preferred_llm_provider: None,
};

const SIPHER_PROFILE: DistributionProfile = DistributionProfile {
    id: "sipher",
    auto_join_default_relay: true,
    default_community_name: Some("Sipher"),
    hosted_communities_enabled: false,
    preferred_agent_runtime: Some("buzz-agent"),
    preferred_llm_provider: Some("ai-gateway"),
};

fn profile_for(distribution: Option<&str>) -> DistributionProfile {
    match distribution {
        Some("sipher") => SIPHER_PROFILE,
        _ => OSS_PROFILE,
    }
}

#[tauri::command]
pub fn get_distribution_profile() -> DistributionProfile {
    profile_for(option_env!("BUZZ_DESKTOP_BUILD_DISTRIBUTION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_distribution_falls_back_to_oss() {
        assert_eq!(profile_for(None), OSS_PROFILE);
        assert_eq!(profile_for(Some("unexpected")), OSS_PROFILE);
    }

    #[test]
    fn sipher_distribution_enables_internal_defaults() {
        assert_eq!(profile_for(Some("sipher")), SIPHER_PROFILE);
    }
}
