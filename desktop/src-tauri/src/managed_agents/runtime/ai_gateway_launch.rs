pub(super) fn resolve(
    effective_command: &str,
    resolved_agent_command: std::path::PathBuf,
    agent_args: Vec<String>,
    effective_provider: Option<&str>,
    effective_env: &std::collections::BTreeMap<String, String>,
) -> Result<super::super::AiGatewayLaunchSpec, String> {
    let uses_ai_gateway = super::super::is_ai_gateway_provider(effective_provider);
    let gateway_profile = if uses_ai_gateway {
        super::super::selected_profile_for_env(effective_env)?
    } else {
        "prod".to_string()
    };
    let gateway_command = uses_ai_gateway
        .then(|| super::super::resolve_command("ai-gateway"))
        .flatten();
    build(
        effective_command,
        resolved_agent_command,
        agent_args,
        effective_provider,
        gateway_command,
        &gateway_profile,
    )
}

pub(super) fn build(
    effective_command: &str,
    resolved_agent_command: std::path::PathBuf,
    agent_args: Vec<String>,
    effective_provider: Option<&str>,
    gateway_command: Option<std::path::PathBuf>,
    gateway_profile: &str,
) -> Result<super::super::AiGatewayLaunchSpec, String> {
    super::super::validate_ai_gateway_runtime(effective_provider, effective_command)?;
    if super::super::is_ai_gateway_provider(effective_provider) {
        if let Some(gateway_command) = gateway_command {
            return super::super::ai_gateway::build_ai_gateway_launch_spec(
                gateway_command,
                gateway_profile,
                resolved_agent_command,
                &agent_args,
            );
        }
        // Readiness puts buzz-acp into setup-listener mode when the CLI is
        // missing. Keep a valid fallback contract so the harness can surface
        // the installation action without launching the unwrapped agent.
    }
    Ok(super::super::AiGatewayLaunchSpec {
        command: resolved_agent_command,
        args: agent_args,
        profile: gateway_profile.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn wraps_only_buzz_agent_ai_gateway_launches() {
        let wrapped = super::build(
            "buzz-agent",
            "/Applications/Buzz.app/Contents/MacOS/buzz-agent".into(),
            vec!["--verbose".into()],
            Some("ai-gateway"),
            Some("/opt/homebrew/bin/ai-gateway".into()),
            "prod",
        )
        .expect("gateway launch");
        assert_eq!(
            wrapped.command,
            std::path::PathBuf::from("/opt/homebrew/bin/ai-gateway")
        );
        assert_eq!(
            wrapped.args,
            vec![
                "--prod",
                "run",
                "/Applications/Buzz.app/Contents/MacOS/buzz-agent",
                "--verbose",
                "--expose",
                "openai",
            ]
        );

        let direct = super::build(
            "buzz-agent",
            "/Applications/Buzz.app/Contents/MacOS/buzz-agent".into(),
            vec![],
            Some("anthropic"),
            None,
            "prod",
        )
        .expect("direct launch");
        assert_eq!(
            direct.command,
            std::path::PathBuf::from("/Applications/Buzz.app/Contents/MacOS/buzz-agent")
        );
        assert!(direct.args.is_empty());

        let setup_fallback = super::build(
            "buzz-agent",
            "/Applications/Buzz.app/Contents/MacOS/buzz-agent".into(),
            vec![],
            Some("ai-gateway"),
            None,
            "prod",
        )
        .expect("missing gateway is handled by setup-listener readiness");
        assert_eq!(
            setup_fallback.command,
            std::path::PathBuf::from("/Applications/Buzz.app/Contents/MacOS/buzz-agent")
        );
    }
}
