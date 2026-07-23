use std::collections::BTreeMap;
use std::process::Command;

pub(crate) const AI_GATEWAY_PROVIDER_ID: &str = "ai-gateway";
const DEFAULT_PROFILE: &str = "prod";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiGatewayConfig {
    pub(crate) profile: String,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiGatewayStatus {
    profile: String,
    base_url: String,
}

fn validate_http_base_url(value: &str, source: &str) -> Result<(), String> {
    let url =
        url::Url::parse(value).map_err(|_| format!("{source} returned an invalid base URL"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!("{source} must be an absolute HTTP(S) URL"));
    }
    Ok(())
}

impl AiGatewayConfig {
    pub(crate) fn openai_compat_env(&self) -> BTreeMap<String, String> {
        let base_url = self.base_url.trim().trim_end_matches('/');
        let versioned_base_url = if base_url.ends_with("/v1") {
            base_url.to_string()
        } else {
            format!("{base_url}/v1")
        };
        BTreeMap::from([
            ("OPENAI_COMPAT_API_KEY".to_string(), self.api_key.clone()),
            ("OPENAI_COMPAT_BASE_URL".to_string(), versioned_base_url),
            ("OPENAI_COMPAT_API".to_string(), "chat".to_string()),
        ])
    }
}

pub(crate) fn is_ai_gateway_provider(provider: Option<&str>) -> bool {
    provider.is_some_and(|value| value.trim().eq_ignore_ascii_case(AI_GATEWAY_PROVIDER_ID))
}

pub(crate) fn ai_gateway_models_request(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    provider: Option<&str>,
) -> reqwest::RequestBuilder {
    let request = client.get(url).bearer_auth(api_key);
    if is_ai_gateway_provider(provider) {
        request.header("X-AI-Gateway-Models-Detail", "full")
    } else {
        request
    }
}

fn parse_offline_status(stdout: &str) -> Result<AiGatewayStatus, String> {
    let status_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("profile="))
        .ok_or_else(|| "AI Gateway returned an unreadable offline status".to_string())?;
    let fields = status_line
        .split_whitespace()
        .filter_map(|field| field.split_once('='))
        .collect::<BTreeMap<_, _>>();
    let profile = fields
        .get("profile")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "AI Gateway offline status did not include a profile".to_string())?;
    let base_url = fields
        .get("base_url")
        .map(|value| value.trim().trim_end_matches('/'))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "AI Gateway offline status did not include a base URL".to_string())?;

    validate_http_base_url(base_url, "AI Gateway offline status")?;

    Ok(AiGatewayStatus {
        profile: profile.to_string(),
        base_url: base_url.to_string(),
    })
}

fn selected_profile() -> Result<String, String> {
    let profile = std::env::var("AI_GATEWAY_PROFILE")
        .unwrap_or_else(|_| DEFAULT_PROFILE.to_string())
        .trim()
        .to_ascii_lowercase();
    match profile.as_str() {
        "dev" | "prod" => Ok(profile),
        _ => Err("AI_GATEWAY_PROFILE must be either dev or prod".to_string()),
    }
}

fn discover_status(profile: &str) -> Result<AiGatewayStatus, String> {
    let executable = super::resolve_command("ai-gateway").ok_or_else(|| {
        "AI Gateway is not installed or is not available on PATH; install it and run `ai-gateway login`"
            .to_string()
    })?;
    let profile_flag = if profile == "dev" { "--dev" } else { "--prod" };
    let output = Command::new(executable)
        .args([profile_flag, "status", "--offline"])
        .output()
        .map_err(|_| "Could not read the local AI Gateway profile".to_string())?;
    if !output.status.success() {
        return Err(format!(
            "AI Gateway profile `{profile}` is not authenticated; run `ai-gateway {profile_flag} login`"
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "AI Gateway returned non-UTF-8 status output".to_string())?;
    parse_offline_status(&stdout)
}

#[cfg(feature = "system-keyring")]
fn key_from_keyring(profile: &str) -> Option<String> {
    ["ai-gateway", "cliproxy"].into_iter().find_map(|service| {
        keyring::Entry::new(service, profile)
            .ok()
            .and_then(|entry| entry.get_password().ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(not(feature = "system-keyring"))]
fn key_from_keyring(_profile: &str) -> Option<String> {
    None
}

fn key_from_file(profile: &str) -> Option<String> {
    let config_dir = dirs::config_dir()?;
    ["ai-gateway", "cliproxy"].into_iter().find_map(|service| {
        let path = config_dir.join(service).join("secrets.json");
        let raw = std::fs::read(path).ok()?;
        let secrets = serde_json::from_slice::<BTreeMap<String, String>>(&raw).ok()?;
        secrets
            .get(profile)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn resolve_key(profile: &str) -> Result<String, String> {
    std::env::var("AI_GATEWAY_API_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| key_from_keyring(profile))
        .or_else(|| key_from_file(profile))
        .ok_or_else(|| {
            format!(
                "AI Gateway profile `{profile}` has no stored credential; run `ai-gateway login`"
            )
        })
}

pub(crate) fn resolve_ai_gateway_config() -> Result<AiGatewayConfig, String> {
    let profile = selected_profile()?;
    let status = discover_status(&profile)?;
    let base_url = std::env::var("AI_GATEWAY_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(status.base_url);
    validate_http_base_url(&base_url, "AI_GATEWAY_BASE_URL")?;
    let api_key = resolve_key(&status.profile)?;

    Ok(AiGatewayConfig {
        profile: status.profile,
        base_url,
        api_key,
    })
}

pub(crate) fn apply_ai_gateway_env(
    env: &mut BTreeMap<String, String>,
    provider: Option<&str>,
) -> Result<(), String> {
    if !is_ai_gateway_provider(provider) {
        return Ok(());
    }
    env.extend(resolve_ai_gateway_config()?.openai_compat_env());
    Ok(())
}

pub(crate) fn validate_ai_gateway_runtime(
    provider: Option<&str>,
    runtime_id: &str,
) -> Result<(), String> {
    let executable_name = runtime_id
        .trim()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let is_buzz_agent = executable_name.eq_ignore_ascii_case("buzz-agent")
        || executable_name.eq_ignore_ascii_case("buzz-agent.exe");
    if is_ai_gateway_provider(provider) && !is_buzz_agent {
        return Err("AI Gateway is only supported by buzz-agent".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_offline_status_without_exposing_key_prefix() {
        let status = parse_offline_status(
            "profile=prod base_url=https://gateway.example key=present key_prefix=secret session=present\ncheck=skipped (offline)",
        )
        .expect("valid status");

        assert_eq!(status.profile, "prod");
        assert_eq!(status.base_url, "https://gateway.example");
    }

    #[test]
    fn rejects_non_http_gateway_urls() {
        let error = parse_offline_status("profile=prod base_url=file:///tmp/gateway")
            .expect_err("non-HTTP gateway URL must fail closed");

        assert!(error.contains("HTTP(S)"), "{error}");
    }

    #[test]
    fn builds_openai_compat_environment_for_buzz_agent() {
        let config = AiGatewayConfig {
            profile: "dev".to_string(),
            base_url: "http://localhost:8317".to_string(),
            api_key: "gateway-secret".to_string(),
        };

        let env = config.openai_compat_env();

        assert_eq!(
            env.get("OPENAI_COMPAT_BASE_URL").map(String::as_str),
            Some("http://localhost:8317/v1")
        );
        assert_eq!(
            env.get("OPENAI_COMPAT_API_KEY").map(String::as_str),
            Some("gateway-secret")
        );
        assert_eq!(
            env.get("OPENAI_COMPAT_API").map(String::as_str),
            Some("chat")
        );
    }

    #[test]
    fn rejects_ai_gateway_for_non_bundled_agent_runtime_before_secret_lookup() {
        let error = validate_ai_gateway_runtime(Some("ai-gateway"), "goose")
            .expect_err("unsupported runtime must fail closed");

        assert!(error.contains("only supported by buzz-agent"), "{error}");
    }

    #[test]
    fn accepts_bundled_buzz_agent_executable_paths() {
        validate_ai_gateway_runtime(
            Some("ai-gateway"),
            "/Applications/Buzz.app/Contents/MacOS/buzz-agent",
        )
        .expect("macOS bundled path");
        validate_ai_gateway_runtime(Some("ai-gateway"), r"C:\Program Files\Buzz\buzz-agent.exe")
            .expect("Windows bundled path");
    }
}
