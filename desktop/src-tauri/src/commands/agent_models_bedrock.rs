use std::collections::BTreeMap;

use crate::managed_agents::{AgentModelInfo, AgentModelsResponse};

use super::{env_or_process_value, DiscoveryProvider};

// ---------------------------------------------------------------------------
// Bedrock model discovery
// ---------------------------------------------------------------------------
//
// Delegates to buzz_agent_pkg::catalog_bedrock::discover_bedrock_models,
// which loads AWS credentials from the env and signs requests with SigV4.

pub(super) fn is_bedrock_provider(provider: Option<&str>) -> bool {
    matches!(
        provider
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("bedrock")
    )
}

pub(super) async fn discover_bedrock_models(
    _client: &reqwest::Client,
    provider: &DiscoveryProvider,
    env: &BTreeMap<String, String>,
    selected_model: Option<String>,
) -> Result<Option<AgentModelsResponse>, String> {
    let provider_str = match provider.as_deref() {
        Some(p) if is_bedrock_provider(Some(p)) => p,
        _ => return Ok(None),
    };

    let region = env_or_process_value(env, "AWS_REGION")
        .or_else(|| env_or_process_value(env, "AWS_DEFAULT_REGION"));
    let region = match region {
        Some(r) => r,
        None => return Ok(None), // no region → fall through to subprocess
    };

    let base_url = format!("https://bedrock-runtime.{region}.amazonaws.com");
    let cfg = buzz_agent_pkg::config::Config::for_discovery(
        buzz_agent_pkg::config::Provider::Bedrock,
        String::new(), // no api_key for Bedrock (SigV4)
        base_url,
    );

    let entries = match buzz_agent_pkg::discover_bedrock_models(&cfg).await {
        Ok(e) => e,
        Err(e) => {
            let msg = e.to_string();
            return Err(format!("Bedrock model discovery failed: {msg}"));
        }
    };

    if entries.is_empty() {
        return Err("Bedrock model discovery returned no models".to_string());
    }

    let models = entries
        .into_iter()
        .map(|e| AgentModelInfo {
            id: e.id,
            name: Some(e.name),
            description: None,
        })
        .collect();

    Ok(Some(AgentModelsResponse {
        agent_name: provider_str.trim().to_string(),
        agent_version: "models-api".to_string(),
        models,
        agent_default_model: None,
        selected_model,
        supports_switching: true,
    }))
}
