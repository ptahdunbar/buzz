//! Bedrock provider readiness tests.
//!
//! Bedrock's credential model has no single required env key — AWS's default
//! credential provider chain (see `buzz_agent_pkg::sigv4::load_aws_credentials`)
//! accepts static access keys, OR an `AWS_PROFILE`, OR an ambient IAM role
//! (EC2 instance profile / IMDS, ECS task role, EKS IRSA) — mutually exclusive
//! sources, so `buzz_agent_requirements`'s bedrock branch checks each source
//! independently rather than requiring one fixed key.
//!
//! Included from `readiness.rs` via `#[path]`; `super::*` therefore resolves
//! against that module, matching the `readiness_goose_file_config_tests.rs`
//! convention.

use std::collections::BTreeMap;

use super::*;
use crate::managed_agents::discovery::known_acp_runtime_exact;

fn make_env(command: &str, env: BTreeMap<String, String>) -> EffectiveAgentEnv {
    let runtime = known_acp_runtime_exact(command);
    EffectiveAgentEnv {
        env,
        config_file_path: runtime.and_then(|r| r.config_file_path),
        effective_command: command.to_string(),
    }
}

fn env_with(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn buzz_agent_bedrock_with_static_keys_is_ready() {
    let env = make_env(
        "buzz-agent",
        env_with(&[
            ("BUZZ_AGENT_PROVIDER", "bedrock"),
            ("BEDROCK_MODEL", "anthropic.claude-3-5-sonnet-20241022-v2:0"),
            ("AWS_ACCESS_KEY_ID", "AKIDEXAMPLE"),
            ("AWS_REGION", "us-east-1"),
        ]),
    );
    let result = agent_readiness(&env);
    assert!(
        result.is_ready(),
        "bedrock with static access keys and region should be ready"
    );
}

#[test]
fn buzz_agent_bedrock_with_aws_profile_is_ready() {
    // AWS_PROFILE (~/.aws/credentials, SSO, credential_process) is a
    // valid credential source for the default provider chain and must
    // not be flagged as missing AWS_ACCESS_KEY_ID.
    let env = make_env(
        "buzz-agent",
        env_with(&[
            ("BUZZ_AGENT_PROVIDER", "bedrock"),
            ("BEDROCK_MODEL", "anthropic.claude-3-5-sonnet-20241022-v2:0"),
            ("AWS_PROFILE", "buzz-bedrock"),
            ("AWS_REGION", "us-east-1"),
        ]),
    );
    let result = agent_readiness(&env);
    assert!(
        result.is_ready(),
        "bedrock with AWS_PROFILE and no static keys should be ready"
    );
}

#[test]
fn buzz_agent_bedrock_with_ecs_container_role_is_ready() {
    let env = make_env(
        "buzz-agent",
        env_with(&[
            ("BUZZ_AGENT_PROVIDER", "bedrock"),
            ("BEDROCK_MODEL", "anthropic.claude-3-5-sonnet-20241022-v2:0"),
            (
                "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI",
                "/v2/credentials/example",
            ),
            ("AWS_REGION", "us-east-1"),
        ]),
    );
    let result = agent_readiness(&env);
    assert!(
        result.is_ready(),
        "bedrock with an ECS task role and no static keys should be ready"
    );
}

#[test]
fn buzz_agent_bedrock_with_eks_irsa_is_ready() {
    let env = make_env(
        "buzz-agent",
        env_with(&[
            ("BUZZ_AGENT_PROVIDER", "bedrock"),
            ("BEDROCK_MODEL", "anthropic.claude-3-5-sonnet-20241022-v2:0"),
            (
                "AWS_ROLE_ARN",
                "arn:aws:iam::123456789012:role/buzz-bedrock",
            ),
            (
                "AWS_WEB_IDENTITY_TOKEN_FILE",
                "/var/run/secrets/eks.amazonaws.com/serviceaccount/token",
            ),
            ("AWS_REGION", "us-east-1"),
        ]),
    );
    let result = agent_readiness(&env);
    assert!(
        result.is_ready(),
        "bedrock with EKS IRSA env vars and no static keys should be ready"
    );
}

#[test]
fn buzz_agent_bedrock_missing_all_credential_sources_returns_not_ready() {
    let env = make_env(
        "buzz-agent",
        env_with(&[
            ("BUZZ_AGENT_PROVIDER", "bedrock"),
            ("BEDROCK_MODEL", "anthropic.claude-3-5-sonnet-20241022-v2:0"),
            ("AWS_REGION", "us-east-1"),
        ]),
    );
    let result = agent_readiness(&env);
    assert!(!result.is_ready());
    assert!(result.requirements().contains(&Requirement::EnvKey {
        key: "AWS_ACCESS_KEY_ID".to_string()
    }));
}

#[test]
fn buzz_agent_bedrock_missing_region_returns_not_ready() {
    let env = make_env(
        "buzz-agent",
        env_with(&[
            ("BUZZ_AGENT_PROVIDER", "bedrock"),
            ("BEDROCK_MODEL", "anthropic.claude-3-5-sonnet-20241022-v2:0"),
            ("AWS_ACCESS_KEY_ID", "AKIDEXAMPLE"),
        ]),
    );
    let result = agent_readiness(&env);
    assert!(!result.is_ready());
    assert!(result.requirements().contains(&Requirement::EnvKey {
        key: "AWS_REGION".to_string()
    }));
}

#[test]
fn buzz_agent_bedrock_default_region_satisfies_region_requirement() {
    let env = make_env(
        "buzz-agent",
        env_with(&[
            ("BUZZ_AGENT_PROVIDER", "bedrock"),
            ("BEDROCK_MODEL", "anthropic.claude-3-5-sonnet-20241022-v2:0"),
            ("AWS_ACCESS_KEY_ID", "AKIDEXAMPLE"),
            ("AWS_DEFAULT_REGION", "us-west-2"),
        ]),
    );
    let result = agent_readiness(&env);
    assert!(
        result.is_ready(),
        "AWS_DEFAULT_REGION fallback should satisfy the region requirement"
    );
}
