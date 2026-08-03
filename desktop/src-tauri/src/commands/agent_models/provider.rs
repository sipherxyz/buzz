use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct OpenAiModelListResponse {
    pub(super) data: Vec<OpenAiModelListItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OpenAiModelListItem {
    pub(super) id: String,
    #[serde(default)]
    pub(super) created: Option<i64>,
    #[serde(default)]
    pub(super) display_name: Option<String>,
}

pub(super) fn is_openai_compatible(provider: Option<&str>) -> bool {
    matches!(
        provider
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("openai" | "openai-compat")
    )
}
