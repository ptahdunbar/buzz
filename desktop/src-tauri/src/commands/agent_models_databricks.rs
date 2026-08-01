use std::collections::BTreeMap;

use crate::managed_agents::{AgentModelInfo, AgentModelsResponse};

use super::{env_or_process_value, redaction_env_with_value, DiscoveryProvider};

// ---------------------------------------------------------------------------
// Databricks model discovery (v1 + v2)
// ---------------------------------------------------------------------------
//
// Delegates to buzz_agent_pkg::catalog::discover_databricks_models, which
// acquires auth in-process via build_token_source:
//   - Static bearer (DATABRICKS_TOKEN): returned immediately.
//   - PKCE cache hit: returned from disk without a browser flow.
//   - No token, no cache: returns Err(LlmAuth) → we return Ok(None) and fall
//     through to run_agent_models_command. Never hangs, never opens a browser.

pub(super) fn is_databricks_provider(provider: Option<&str>) -> bool {
    matches!(
        provider
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("databricks" | "databricks_v2" | "databricks-v2")
    )
}

fn databricks_agent_provider(provider: &str) -> buzz_agent_pkg::config::Provider {
    if provider.trim().eq_ignore_ascii_case("databricks_v2")
        || provider.trim().eq_ignore_ascii_case("databricks-v2")
    {
        buzz_agent_pkg::config::Provider::DatabricksV2
    } else {
        buzz_agent_pkg::config::Provider::Databricks
    }
}

pub(super) async fn discover_databricks_models(
    _client: &reqwest::Client,
    provider: &DiscoveryProvider,
    env: &BTreeMap<String, String>,
    selected_model: Option<String>,
) -> Result<Option<AgentModelsResponse>, String> {
    let provider_str = match provider.as_deref() {
        Some(p) if is_databricks_provider(Some(p)) => p,
        _ => return Ok(None),
    };

    let host = match env_or_process_value(env, "DATABRICKS_HOST") {
        Some(h) => h,
        None => return Ok(None), // no host → fall through to subprocess
    };

    // api_key = DATABRICKS_TOKEN (empty string = use PKCE cache).
    let api_key = env_or_process_value(env, "DATABRICKS_TOKEN").unwrap_or_default();

    let agent_provider = databricks_agent_provider(provider_str);
    let cfg = buzz_agent_pkg::config::Config::for_discovery(agent_provider, api_key, host);

    // Build a redaction env so the token never appears in surfaced errors.
    let token_for_redact = env_or_process_value(env, "DATABRICKS_TOKEN").unwrap_or_default();
    let redaction_env = redaction_env_with_value(env, "DATABRICKS_TOKEN", &token_for_redact);

    let entries = match buzz_agent_pkg::discover_databricks_models(&cfg).await {
        Ok(e) => e,
        Err(buzz_agent_pkg::AgentError::LlmAuth(_)) => {
            // No token + no PKCE cache → fall through to subprocess.
            return Ok(None);
        }
        Err(e) => {
            let msg = crate::managed_agents::redact_env_values_in(&e.to_string(), &redaction_env);
            return Err(format!("Databricks model discovery failed: {msg}"));
        }
    };

    if entries.is_empty() {
        return Err("Databricks model discovery returned no models".to_string());
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
