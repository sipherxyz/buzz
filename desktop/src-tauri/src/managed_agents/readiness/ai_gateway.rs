use crate::managed_agents::{AcpAvailabilityStatus, AiGatewayProbeError, AiGatewayStatus};

use super::{EffectiveAgentEnv, Requirement};

pub(super) fn requirements(effective: &EffectiveAgentEnv) -> Vec<Requirement> {
    requirements_for_probe(crate::managed_agents::probe_ai_gateway_status(
        &effective.env,
    ))
}

fn requirements_for_probe(
    result: Result<AiGatewayStatus, AiGatewayProbeError>,
) -> Vec<Requirement> {
    let requirement = match result {
        Ok(AiGatewayStatus::Ready { .. }) => return vec![],
        Ok(AiGatewayStatus::LoggedOut { profile, .. }) => Requirement::CliLogin {
            probe_args: vec![
                "ai-gateway".to_string(),
                format!("--{profile}"),
                "status".to_string(),
                "--offline".to_string(),
            ],
            setup_copy: format!("run `ai-gateway --{profile} login`"),
            availability: AcpAvailabilityStatus::NotInstalled,
        },
        Err(AiGatewayProbeError::MissingExecutable) => Requirement::CliLogin {
            probe_args: vec![
                "ai-gateway".to_string(),
                "--prod".to_string(),
                "status".to_string(),
                "--offline".to_string(),
            ],
            setup_copy: "install AI Gateway, then run `ai-gateway --prod login`".to_string(),
            availability: AcpAvailabilityStatus::CliMissing,
        },
        Err(error) => {
            let diagnostic = match error {
                AiGatewayProbeError::InvalidProfile(message)
                | AiGatewayProbeError::InvalidOutput(message)
                | AiGatewayProbeError::CommandFailed(message) => message,
                AiGatewayProbeError::MissingExecutable => unreachable!(),
            };
            Requirement::CliLogin {
                probe_args: vec![
                    "ai-gateway".to_string(),
                    "--prod".to_string(),
                    "status".to_string(),
                    "--offline".to_string(),
                ],
                setup_copy: format!(
                    "AI Gateway readiness check failed: {diagnostic}. Fix the local gateway and retry"
                ),
                availability: AcpAvailabilityStatus::AdapterMissing,
            }
        }
    };
    vec![requirement]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_states_map_to_actionable_readiness() {
        assert!(requirements_for_probe(Ok(AiGatewayStatus::Ready {
            profile: "prod".into(),
            base_url: "http://127.0.0.1:8080".into(),
        }))
        .is_empty());

        let logged_out = requirements_for_probe(Ok(AiGatewayStatus::LoggedOut {
            profile: "prod".into(),
            base_url: "http://127.0.0.1:8080".into(),
        }));
        assert!(matches!(
            logged_out.as_slice(),
            [Requirement::CliLogin {
                availability: AcpAvailabilityStatus::NotInstalled,
                ..
            }]
        ));

        let missing = requirements_for_probe(Err(AiGatewayProbeError::MissingExecutable));
        assert!(matches!(
            missing.as_slice(),
            [Requirement::CliLogin {
                availability: AcpAvailabilityStatus::CliMissing,
                ..
            }]
        ));

        let failed = requirements_for_probe(Err(AiGatewayProbeError::InvalidOutput(
            "unsupported response".into(),
        )));
        assert!(matches!(
            failed.as_slice(),
            [Requirement::CliLogin {
                availability: AcpAvailabilityStatus::AdapterMissing,
                ..
            }]
        ));
    }
}
