//! AWS SigV4 request signing for Bedrock API calls.
//!
//! Uses the `aws-sigv4` crate from the official Rust SDK to sign HTTP
//! requests with Signature Version 4, which AWS Bedrock requires instead
//! of bearer tokens.
//!
//! Credentials are loaded via `aws-config`'s standard default provider
//! chain, so all the usual sources work: `AWS_ACCESS_KEY_ID` /
//! `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN` env vars, `AWS_PROFILE`
//! (`~/.aws/credentials` and `~/.aws/config`), AWS SSO, and IAM roles
//! (IMDS on EC2, ECS task roles, EKS IRSA).

use aws_config::BehaviorVersion;
use aws_credential_types::provider::ProvideCredentials;
use aws_credential_types::Credentials as AwsCreds;
use aws_sigv4::http_request::{sign, SignableBody, SignableRequest, SigningSettings};
use aws_sigv4::sign::v4;
use aws_smithy_runtime_api::client::identity::Identity;
use http::Request;
use std::time::SystemTime;

/// AWS credentials used for SigV4 signing.
#[derive(Debug, Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

/// Sign an HTTP request with AWS SigV4.
///
/// `service` is typically `"bedrock"`. `region` is the AWS region
/// (e.g. `"us-east-1"`).
pub fn sign_request(
    mut request: Request<Vec<u8>>,
    creds: &AwsCredentials,
    service: &str,
    region: &str,
) -> Result<Request<Vec<u8>>, String> {
    let identity: Identity = AwsCreds::new(
        &creds.access_key_id,
        &creds.secret_access_key,
        creds.session_token.clone(),
        None,
        "buzz-agent",
    )
    .into();

    let uri_str = request.uri().to_string();
    let signable = SignableRequest::new(
        request.method().as_str(),
        uri_str.as_str(),
        request
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str(), v.to_str().unwrap_or_default())),
        SignableBody::Bytes(request.body()),
    )
    .map_err(|e| format!("signable request: {e}"))?;

    let settings = SigningSettings::default();
    let params: aws_sigv4::http_request::SigningParams<'_> = v4::SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name(service)
        .time(SystemTime::now())
        .settings(settings)
        .build()
        .map_err(|e| format!("signing params: {e}"))?
        .into();

    let signing_output = sign(signable, &params).map_err(|e| format!("signing: {e}"))?;

    let (instructions, _signature) = signing_output.into_parts();
    instructions.apply_to_request_http1x(&mut request);

    Ok(request)
}

/// Resolve AWS credentials for signing a Bedrock request in `region`.
///
/// Delegates to `aws-config`'s standard default provider chain, which tries
/// (in order): environment variables (`AWS_ACCESS_KEY_ID` /
/// `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`), the profile named by
/// `AWS_PROFILE` (or `default`) in `~/.aws/credentials` and
/// `~/.aws/config` — including `sso_start_url` profiles and
/// `credential_process` — then container/IMDS instance-role credentials
/// (ECS task roles, EKS IRSA, EC2 instance profiles).
///
/// `region` seeds the loader so region-sensitive providers (e.g. IMDS,
/// STS) target the right endpoint; it does not restrict which credential
/// source is tried.
pub async fn load_aws_credentials(region: &str) -> Result<AwsCredentials, String> {
    let sdk_config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(region.to_string()))
        .load()
        .await;

    let provider = sdk_config
        .credentials_provider()
        .ok_or_else(|| "config: no AWS credential provider resolved for Bedrock".to_string())?;

    let creds = provider
        .provide_credentials()
        .await
        .map_err(|e| format!("config: failed to resolve AWS credentials for Bedrock: {e}"))?;

    Ok(AwsCredentials {
        access_key_id: creds.access_key_id().to_string(),
        secret_access_key: creds.secret_access_key().to_string(),
        session_token: creds.session_token().map(str::to_string),
    })
}

/// Extract the AWS region from a Bedrock runtime base URL.
///
/// Accepts:
/// - `https://bedrock-runtime.{region}.amazonaws.com`
/// - `https://bedrock-runtime.{region}.amazonaws.com/v1`
pub fn parse_bedrock_region(base_url: &str) -> Result<String, String> {
    let rest = base_url
        .strip_prefix("https://bedrock-runtime.")
        .ok_or_else(|| format!("Bedrock: could not extract region from base_url: {base_url}"))?;
    let region = rest
        .strip_suffix(".amazonaws.com")
        .or_else(|| rest.strip_suffix(".amazonaws.com/v1"))
        .ok_or_else(|| format!("Bedrock: could not extract region from base_url: {base_url}"))?;
    Ok(region.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_request_adds_authorization_header() {
        let creds = AwsCredentials {
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            session_token: None,
        };
        let body = r#"{"messages":[]}"#;
        let req = Request::builder()
            .uri("https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-5-sonnet-20241022-v2:0/converse")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(body.as_bytes().to_vec())
            .unwrap();
        let signed = sign_request(req, &creds, "bedrock", "us-east-1").unwrap();
        let auth = signed
            .headers()
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            auth.starts_with("AWS4-HMAC-SHA256"),
            "expected SigV4 auth header, got: {auth}"
        );
        assert!(
            auth.contains("us-east-1/bedrock/"),
            "expected region/service in credential scope, got: {auth}"
        );
    }

    #[test]
    fn test_sign_request_with_session_token() {
        let creds = AwsCredentials {
            access_key_id: "AKIDEXAMPLE".into(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY".into(),
            session_token: Some("IQoJb3JpZ2luX2IQoJb3JpZ2luX2IQ".into()),
        };
        let body = r#"{"messages":[]}"#;
        let req = Request::builder()
            .uri("https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-5-sonnet-20241022-v2:0/converse")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(body.as_bytes().to_vec())
            .unwrap();
        let signed = sign_request(req, &creds, "bedrock", "us-east-1").unwrap();
        let auth = signed
            .headers()
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(auth.starts_with("AWS4-HMAC-SHA256"));
        let st = signed
            .headers()
            .get("x-amz-security-token")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(st, "IQoJb3JpZ2luX2IQoJb3JpZ2luX2IQ");
    }

    #[test]
    fn test_parse_bedrock_region_standard() {
        let region =
            parse_bedrock_region("https://bedrock-runtime.us-east-1.amazonaws.com").unwrap();
        assert_eq!(region, "us-east-1");
    }

    #[test]
    fn test_parse_bedrock_region_with_v1_suffix() {
        let region =
            parse_bedrock_region("https://bedrock-runtime.eu-west-1.amazonaws.com/v1").unwrap();
        assert_eq!(region, "eu-west-1");
    }

    #[test]
    fn test_parse_bedrock_region_invalid_url() {
        let result = parse_bedrock_region("https://api.openai.com/v1");
        assert!(result.is_err());
    }

    // Serializes env-var-mutating credential tests so they don't race each
    // other or the ambient env inside the same test binary.
    static ENV_MUTATION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn test_load_credentials_from_env() {
        let _guard = ENV_MUTATION_LOCK.lock().await;
        let old_key = std::env::var("AWS_ACCESS_KEY_ID").ok();
        let old_secret = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
        let old_token = std::env::var("AWS_SESSION_TOKEN").ok();
        let old_profile = std::env::var("AWS_PROFILE").ok();

        std::env::set_var("AWS_ACCESS_KEY_ID", "test-key");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test-secret");
        std::env::remove_var("AWS_SESSION_TOKEN");
        // Env credentials take priority over profile lookups in the default
        // chain, but clear AWS_PROFILE too so a real dev machine's profile
        // can't be consulted (and can't flake the test on a slow lookup).
        std::env::remove_var("AWS_PROFILE");

        let creds = load_aws_credentials("us-east-1").await.unwrap();
        assert_eq!(creds.access_key_id, "test-key");
        assert_eq!(creds.secret_access_key, "test-secret");
        assert!(creds.session_token.is_none());

        // Restore original env vars
        if let Some(k) = old_key {
            std::env::set_var("AWS_ACCESS_KEY_ID", k);
        } else {
            std::env::remove_var("AWS_ACCESS_KEY_ID");
        }
        if let Some(s) = old_secret {
            std::env::set_var("AWS_SECRET_ACCESS_KEY", s);
        } else {
            std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        }
        if let Some(t) = old_token {
            std::env::set_var("AWS_SESSION_TOKEN", t);
        } else {
            std::env::remove_var("AWS_SESSION_TOKEN");
        }
        if let Some(p) = old_profile {
            std::env::set_var("AWS_PROFILE", p);
        } else {
            std::env::remove_var("AWS_PROFILE");
        }
    }

    #[tokio::test]
    async fn test_load_credentials_with_session_token_from_env() {
        let _guard = ENV_MUTATION_LOCK.lock().await;
        let old_key = std::env::var("AWS_ACCESS_KEY_ID").ok();
        let old_secret = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
        let old_token = std::env::var("AWS_SESSION_TOKEN").ok();
        let old_profile = std::env::var("AWS_PROFILE").ok();

        std::env::set_var("AWS_ACCESS_KEY_ID", "test-key");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test-secret");
        std::env::set_var("AWS_SESSION_TOKEN", "test-session-token");
        std::env::remove_var("AWS_PROFILE");

        let creds = load_aws_credentials("us-east-1").await.unwrap();
        assert_eq!(creds.access_key_id, "test-key");
        assert_eq!(creds.secret_access_key, "test-secret");
        assert_eq!(creds.session_token.as_deref(), Some("test-session-token"));

        if let Some(k) = old_key {
            std::env::set_var("AWS_ACCESS_KEY_ID", k);
        } else {
            std::env::remove_var("AWS_ACCESS_KEY_ID");
        }
        if let Some(s) = old_secret {
            std::env::set_var("AWS_SECRET_ACCESS_KEY", s);
        } else {
            std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        }
        if let Some(t) = old_token {
            std::env::set_var("AWS_SESSION_TOKEN", t);
        } else {
            std::env::remove_var("AWS_SESSION_TOKEN");
        }
        if let Some(p) = old_profile {
            std::env::set_var("AWS_PROFILE", p);
        } else {
            std::env::remove_var("AWS_PROFILE");
        }
    }

    #[tokio::test]
    async fn test_load_credentials_from_named_profile() {
        let _guard = ENV_MUTATION_LOCK.lock().await;
        let old_key = std::env::var("AWS_ACCESS_KEY_ID").ok();
        let old_secret = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
        let old_token = std::env::var("AWS_SESSION_TOKEN").ok();
        let old_profile = std::env::var("AWS_PROFILE").ok();
        let old_shared_creds_file = std::env::var("AWS_SHARED_CREDENTIALS_FILE").ok();
        let old_config_file = std::env::var("AWS_CONFIG_FILE").ok();

        // Clear the env-credentials sources so the chain falls through to
        // the profile provider, and point the profile provider at a fixture
        // credentials file instead of the real ~/.aws/credentials.
        std::env::remove_var("AWS_ACCESS_KEY_ID");
        std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        std::env::remove_var("AWS_SESSION_TOKEN");

        let fixture_dir =
            std::env::temp_dir().join(format!("buzz-agent-sigv4-test-{}", std::process::id()));
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let creds_path = fixture_dir.join("credentials");
        std::fs::write(
            &creds_path,
            "[buzz-test-profile]\naws_access_key_id = profile-key\naws_secret_access_key = profile-secret\n",
        )
        .unwrap();

        std::env::set_var("AWS_SHARED_CREDENTIALS_FILE", &creds_path);
        std::env::set_var("AWS_CONFIG_FILE", fixture_dir.join("config"));
        std::env::set_var("AWS_PROFILE", "buzz-test-profile");

        let creds = load_aws_credentials("us-east-1").await.unwrap();
        assert_eq!(creds.access_key_id, "profile-key");
        assert_eq!(creds.secret_access_key, "profile-secret");

        let _ = std::fs::remove_dir_all(&fixture_dir);

        if let Some(k) = old_key {
            std::env::set_var("AWS_ACCESS_KEY_ID", k);
        } else {
            std::env::remove_var("AWS_ACCESS_KEY_ID");
        }
        if let Some(s) = old_secret {
            std::env::set_var("AWS_SECRET_ACCESS_KEY", s);
        } else {
            std::env::remove_var("AWS_SECRET_ACCESS_KEY");
        }
        if let Some(t) = old_token {
            std::env::set_var("AWS_SESSION_TOKEN", t);
        } else {
            std::env::remove_var("AWS_SESSION_TOKEN");
        }
        if let Some(p) = old_profile {
            std::env::set_var("AWS_PROFILE", p);
        } else {
            std::env::remove_var("AWS_PROFILE");
        }
        if let Some(f) = old_shared_creds_file {
            std::env::set_var("AWS_SHARED_CREDENTIALS_FILE", f);
        } else {
            std::env::remove_var("AWS_SHARED_CREDENTIALS_FILE");
        }
        if let Some(f) = old_config_file {
            std::env::set_var("AWS_CONFIG_FILE", f);
        } else {
            std::env::remove_var("AWS_CONFIG_FILE");
        }
    }
}
