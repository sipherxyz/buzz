use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

pub(crate) const AI_GATEWAY_PROVIDER_ID: &str = "ai-gateway";
const DEFAULT_PROFILE: &str = "prod";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiGatewayModel {
    pub(crate) id: String,
    pub(crate) provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiGatewayLaunchSpec {
    pub(crate) command: PathBuf,
    pub(crate) args: Vec<String>,
    pub(crate) profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiGatewayStatus {
    Ready { profile: String, base_url: String },
    LoggedOut { profile: String, base_url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AiGatewayProbeError {
    MissingExecutable,
    InvalidProfile(String),
    InvalidOutput(String),
    CommandFailed(String),
}

fn validate_http_base_url(value: &str, source: &str) -> Result<(), String> {
    let url =
        url::Url::parse(value).map_err(|_| format!("{source} returned an invalid base URL"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(format!("{source} must be an absolute HTTP(S) URL"));
    }
    Ok(())
}

pub(crate) fn is_ai_gateway_provider(provider: Option<&str>) -> bool {
    provider.is_some_and(|value| value.trim().eq_ignore_ascii_case(AI_GATEWAY_PROVIDER_ID))
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
    match fields.get("key").copied() {
        Some("present") => Ok(AiGatewayStatus::Ready {
            profile: profile.to_string(),
            base_url: base_url.to_string(),
        }),
        Some("missing") => Ok(AiGatewayStatus::LoggedOut {
            profile: profile.to_string(),
            base_url: base_url.to_string(),
        }),
        _ => Err("AI Gateway offline status did not include a recognized key state".to_string()),
    }
}

pub(crate) fn selected_profile_for_env(env: &BTreeMap<String, String>) -> Result<String, String> {
    let profile = env
        .get("AI_GATEWAY_PROFILE")
        .cloned()
        .or_else(|| std::env::var("AI_GATEWAY_PROFILE").ok())
        .unwrap_or_else(|| DEFAULT_PROFILE.to_string())
        .trim()
        .to_ascii_lowercase();
    match profile.as_str() {
        "dev" | "prod" => Ok(profile),
        _ => Err("AI_GATEWAY_PROFILE must be either dev or prod".to_string()),
    }
}

fn profile_flag(profile: &str) -> Result<&'static str, String> {
    match profile {
        "dev" => Ok("--dev"),
        "prod" => Ok("--prod"),
        _ => Err("AI Gateway profile must be either dev or prod".to_string()),
    }
}

fn is_cross_platform_absolute(path: &std::path::Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let value = path.to_string_lossy().as_bytes().to_vec();
    value.first() == Some(&b'/')
        || value.len() >= 3
            && value[0].is_ascii_alphabetic()
            && value[1] == b':'
            && matches!(value[2], b'\\' | b'/')
}

pub(crate) fn build_ai_gateway_launch_spec(
    gateway_command: PathBuf,
    profile: &str,
    agent_command: PathBuf,
    agent_args: &[String],
) -> Result<AiGatewayLaunchSpec, String> {
    if !is_cross_platform_absolute(&gateway_command) {
        return Err("AI Gateway executable path must be absolute".to_string());
    }
    if !is_cross_platform_absolute(&agent_command) {
        return Err("buzz-agent executable path must be absolute".to_string());
    }

    let profile_flag = profile_flag(profile)?;
    let mut args = Vec::with_capacity(agent_args.len() + 5);
    args.push(profile_flag.to_string());
    args.push("run".to_string());
    args.push(agent_command.to_string_lossy().into_owned());
    args.extend(agent_args.iter().cloned());
    args.push("--expose".to_string());
    args.push("openai".to_string());

    Ok(AiGatewayLaunchSpec {
        command: gateway_command,
        args,
        profile: profile.to_string(),
    })
}

pub(crate) fn parse_models_output(stdout: &str) -> Result<Vec<AiGatewayModel>, String> {
    let mut provider: Option<String> = None;
    let mut seen = HashSet::new();
    let mut models = Vec::new();

    for raw_line in stdout.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        if !raw_line.starts_with(char::is_whitespace) && line.ends_with(':') {
            let heading = line.trim_end_matches(':').trim();
            if heading.is_empty() {
                return Err("AI Gateway returned an empty provider heading".to_string());
            }
            provider = Some(heading.to_string());
            continue;
        }
        if let Some(model_id) = line.trim().strip_prefix("- ") {
            let model_id = model_id.trim();
            let current_provider = provider.as_deref().ok_or_else(|| {
                "AI Gateway returned a model before its provider heading".to_string()
            })?;
            if model_id.is_empty() {
                return Err("AI Gateway returned an empty model ID".to_string());
            }
            if seen.insert(model_id.to_string()) {
                models.push(AiGatewayModel {
                    id: model_id.to_string(),
                    provider: current_provider.to_string(),
                });
            }
            continue;
        }
        return Err("AI Gateway returned unreadable model output".to_string());
    }

    if models.is_empty() {
        return Err("AI Gateway returned no models".to_string());
    }
    Ok(models)
}

pub(crate) async fn discover_ai_gateway_models() -> Result<Vec<AiGatewayModel>, String> {
    let profile = selected_profile_for_env(&BTreeMap::new())?;
    let profile_flag = profile_flag(&profile)?;
    let executable = super::resolve_command("ai-gateway").ok_or_else(|| {
        "AI Gateway is not installed or is not available on PATH; install it and run `ai-gateway login`"
            .to_string()
    })?;
    let mut command = tokio::process::Command::new(executable);
    command.args([profile_flag, "models"]).kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(30), command.output())
        .await
        .map_err(|_| "AI Gateway model discovery timed out".to_string())?
        .map_err(|error| format!("Could not launch AI Gateway model discovery: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no stored key") {
            return Err(format!(
                "AI Gateway profile `{profile}` is not authenticated; run `ai-gateway {profile_flag} login`"
            ));
        }
        return Err(format!(
            "AI Gateway model discovery failed with status {}",
            output.status
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "AI Gateway returned non-UTF-8 model output".to_string())?;
    parse_models_output(&stdout)
}

pub(crate) fn probe_ai_gateway_status(
    env: &BTreeMap<String, String>,
) -> Result<AiGatewayStatus, AiGatewayProbeError> {
    let profile = selected_profile_for_env(env).map_err(AiGatewayProbeError::InvalidProfile)?;
    let executable =
        super::resolve_command("ai-gateway").ok_or(AiGatewayProbeError::MissingExecutable)?;
    let profile_flag = profile_flag(&profile).map_err(AiGatewayProbeError::InvalidProfile)?;
    let mut command = Command::new(executable);
    command
        .args([profile_flag, "status", "--offline"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        AiGatewayProbeError::CommandFailed(format!(
            "`ai-gateway status --offline` could not start: {error}"
        ))
    })?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AiGatewayProbeError::CommandFailed(
                    "`ai-gateway status --offline` timed out".to_string(),
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AiGatewayProbeError::CommandFailed(format!(
                    "`ai-gateway status --offline` could not be checked: {error}"
                )));
            }
        }
    };
    let output = child.wait_with_output().map_err(|error| {
        AiGatewayProbeError::CommandFailed(format!(
            "`ai-gateway status --offline` output could not be read: {error}"
        ))
    })?;
    let stdout = String::from_utf8(output.stdout).map_err(|_| {
        AiGatewayProbeError::InvalidOutput(
            "AI Gateway returned non-UTF-8 status output".to_string(),
        )
    })?;
    let status = parse_offline_status(&stdout).map_err(AiGatewayProbeError::InvalidOutput)?;
    if exit_status.success() || matches!(status, AiGatewayStatus::LoggedOut { .. }) {
        Ok(status)
    } else {
        Err(AiGatewayProbeError::CommandFailed(format!(
            "`ai-gateway status --offline` failed with status {}",
            exit_status
        )))
    }
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

        assert_eq!(
            status,
            AiGatewayStatus::Ready {
                profile: "prod".to_string(),
                base_url: "https://gateway.example".to_string(),
            }
        );
    }

    #[test]
    fn parses_logged_out_offline_status() {
        assert_eq!(
            parse_offline_status("profile=prod base_url=http://127.0.0.1:8317 key=missing\n")
                .expect("logged-out status"),
            AiGatewayStatus::LoggedOut {
                profile: "prod".to_string(),
                base_url: "http://127.0.0.1:8317".to_string(),
            }
        );
    }

    #[test]
    fn rejects_non_http_gateway_urls() {
        let error = parse_offline_status("profile=prod base_url=file:///tmp/gateway")
            .expect_err("non-HTTP gateway URL must fail closed");

        assert!(error.contains("HTTP(S)"), "{error}");
    }

    #[test]
    fn parses_grouped_model_output_without_duplicates() {
        let models = parse_models_output(
            "anthropic:\n  - claude-sonnet-4-5\nopenai:\n  - gpt-5.4\n  - claude-sonnet-4-5\n",
        )
        .expect("valid models");

        assert_eq!(
            models,
            vec![
                AiGatewayModel {
                    id: "claude-sonnet-4-5".to_string(),
                    provider: "anthropic".to_string(),
                },
                AiGatewayModel {
                    id: "gpt-5.4".to_string(),
                    provider: "openai".to_string(),
                },
            ]
        );
    }

    #[test]
    fn rejects_empty_or_malformed_model_output() {
        assert!(parse_models_output("No models found.\n").is_err());
        assert!(parse_models_output("  - model-without-provider\n").is_err());
        assert!(parse_models_output("provider:\nunexpected\n").is_err());
    }

    #[test]
    fn builds_posix_gateway_launch_arguments() {
        let spec = build_ai_gateway_launch_spec(
            "/opt/homebrew/bin/ai-gateway".into(),
            "prod",
            "/Applications/Buzz.app/Contents/MacOS/buzz-agent".into(),
            &["--verbose".to_string()],
        )
        .expect("valid launch");

        assert_eq!(
            spec.args,
            vec![
                "--prod",
                "run",
                "/Applications/Buzz.app/Contents/MacOS/buzz-agent",
                "--verbose",
                "--expose",
                "openai",
            ]
        );
    }

    #[test]
    fn builds_windows_gateway_launch_arguments_without_a_shell() {
        let spec = build_ai_gateway_launch_spec(
            r"C:\Tools\ai-gateway.exe".into(),
            "dev",
            r"C:\Program Files\Buzz\buzz-agent.exe".into(),
            &[],
        )
        .expect("valid launch");

        assert_eq!(
            spec.command,
            std::path::PathBuf::from(r"C:\Tools\ai-gateway.exe")
        );
        assert_eq!(
            spec.args,
            vec![
                "--dev",
                "run",
                r"C:\Program Files\Buzz\buzz-agent.exe",
                "--expose",
                "openai",
            ]
        );
    }

    #[test]
    fn rejects_ai_gateway_for_non_bundled_agent_runtime_before_cli_lookup() {
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
