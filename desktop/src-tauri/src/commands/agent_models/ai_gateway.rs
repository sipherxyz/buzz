use super::{AgentModelInfo, AgentModelsResponse};

pub(super) fn models_response(
    models: Vec<crate::managed_agents::AiGatewayModel>,
    selected_model: Option<String>,
) -> AgentModelsResponse {
    AgentModelsResponse {
        agent_name: crate::managed_agents::AI_GATEWAY_PROVIDER_ID.to_string(),
        agent_version: "cli".to_string(),
        models: models
            .into_iter()
            .map(|model| AgentModelInfo {
                id: model.id.clone(),
                name: Some(model.id),
                description: Some(format!("Provider: {}", model.provider)),
            })
            .collect(),
        agent_default_model: None,
        selected_model,
        supports_switching: true,
    }
}

pub(super) async fn discover(
    selected_model: Option<String>,
) -> Result<AgentModelsResponse, String> {
    let models = crate::managed_agents::discover_ai_gateway_models().await?;
    Ok(models_response(models, selected_model))
}
