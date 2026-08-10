//! Amazon Bedrock text generation via the Converse API.
//!
//! This is deliberately the plumbing only: prompt construction and cleanup of
//! the model's reply stay with the caller, because those are exactly the parts
//! that differ between applications.

use crate::aws;
use anyhow::{Result, anyhow};
use aws_config::SdkConfig;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::config::ProvideCredentials;
use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, ConverseOutput, Message};
use log::debug;
use std::env;
use std::future::Future;
use std::time::Duration;

/// Model used when [`MODEL_ID_VAR`] is unset.
pub const DEFAULT_MODEL_ID: &str = "us.amazon.nova-2-lite-v1:0";

/// Environment variable overriding [`DEFAULT_MODEL_ID`], so a model can be
/// swapped per deployment without a rebuild.
pub const MODEL_ID_VAR: &str = "BEDROCK_MODEL_ID";

/// A Bedrock Converse client bound to one model.
#[derive(Debug, Clone)]
pub struct BedrockClient {
    client: Client,
    model_id: String,
}

impl BedrockClient {
    /// Builds a client from the ambient AWS configuration.
    ///
    /// Credential and connectivity problems surface later as per-call errors,
    /// which suits callers that already fall back to a non-AI path when
    /// generation fails. Callers that would rather not construct the client at
    /// all want [`from_env_if_credentialed`](Self::from_env_if_credentialed).
    pub async fn from_env() -> Self {
        Self::new(&aws::config(None).await)
    }

    /// [`from_env`](Self::from_env), but `None` when AWS credentials aren't
    /// configured.
    ///
    /// For binaries that run both credentialed (in Lambda) and not (on a
    /// developer's machine), where the uncredentialed case is ordinary rather
    /// than an error worth reporting.
    pub async fn from_env_if_credentialed() -> Option<Self> {
        let config = aws::config(None).await;

        if !is_credentialed(&config).await {
            return None;
        }

        Some(Self::new(&config))
    }

    /// Builds a client from an already-loaded [`SdkConfig`].
    pub fn new(config: &SdkConfig) -> Self {
        Self {
            client: Client::new(config),
            model_id: resolve_model_id(env::var(MODEL_ID_VAR).ok()),
        }
    }

    /// The model this client sends to.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    /// Sends `prompt` as a single user turn and returns the model's reply
    /// verbatim — untrimmed, so callers own the cleanup.
    pub async fn generate(&self, prompt: &str) -> Result<String> {
        let message = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(prompt.to_string()))
            .build()?;

        let response = self
            .client
            .converse()
            .model_id(&self.model_id)
            .messages(message)
            .send()
            .await?;

        let text = extract_text(response.output())?;
        debug!("Bedrock response ({} chars)", text.len());

        Ok(text)
    }

    /// [`generate`](Self::generate) abandoned after `timeout`.
    ///
    /// For callers on a deadline of their own — a Lambda that still has to
    /// render a response — where a slow model should lose to the fallback
    /// rather than blow the budget.
    pub async fn generate_with_timeout(&self, prompt: &str, timeout: Duration) -> Result<String> {
        with_timeout(timeout, self.generate(prompt)).await
    }
}

/// Whether `config` can actually produce credentials.
///
/// Split out from [`BedrockClient::from_env_if_credentialed`] so the decision
/// is testable against a hand-built [`SdkConfig`] — resolving the real
/// credential chain in a test would depend on the machine running it.
async fn is_credentialed(config: &SdkConfig) -> bool {
    let Some(provider) = config.credentials_provider() else {
        debug!("No AWS credentials provider configured; skipping Bedrock");
        return false;
    };

    if let Err(e) = provider.provide_credentials().await {
        debug!("AWS credentials unavailable; skipping Bedrock: {e}");
        return false;
    }

    true
}

fn resolve_model_id(override_id: Option<String>) -> String {
    override_id.unwrap_or_else(|| DEFAULT_MODEL_ID.to_owned())
}

fn extract_text(output: Option<&ConverseOutput>) -> Result<String> {
    output
        .and_then(|o| o.as_message().ok())
        .and_then(|m| m.content().first())
        .and_then(|b| b.as_text().ok())
        .map(String::to_owned)
        .ok_or_else(|| anyhow!("Unexpected Bedrock response structure"))
}

async fn with_timeout<F, T>(timeout: Duration, future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| anyhow!("Bedrock request timed out after {timeout:?}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_credential_types::provider;
    use aws_credential_types::provider::error::CredentialsError;
    use aws_sdk_bedrockruntime::config::{Credentials, SharedCredentialsProvider};
    use aws_sdk_bedrockruntime::types::{
        CachePointBlock, CachePointType, ContentBlock, ConversationRole, ConverseOutput, Message,
    };

    fn message_output(blocks: Vec<ContentBlock>) -> ConverseOutput {
        let mut message = Message::builder().role(ConversationRole::Assistant);
        for block in blocks {
            message = message.content(block);
        }
        ConverseOutput::Message(message.build().expect("message builds"))
    }

    #[test]
    fn test_resolve_model_id_defaults() {
        assert_eq!(resolve_model_id(None), DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_resolve_model_id_honors_override() {
        assert_eq!(
            resolve_model_id(Some("anthropic.claude".to_string())),
            "anthropic.claude"
        );
    }

    #[test]
    fn test_extract_text_returns_first_text_block() {
        let output = message_output(vec![ContentBlock::Text("a summary".to_string())]);

        assert_eq!(extract_text(Some(&output)).unwrap(), "a summary");
    }

    #[test]
    fn test_extract_text_preserves_whitespace() {
        // Callers do their own trimming and cleanup, so the raw text has to
        // survive this far intact.
        let output = message_output(vec![ContentBlock::Text("  padded  ".to_string())]);

        assert_eq!(extract_text(Some(&output)).unwrap(), "  padded  ");
    }

    #[test]
    fn test_extract_text_rejects_missing_output() {
        assert!(extract_text(None).is_err());
    }

    #[test]
    fn test_extract_text_rejects_non_text_leading_block() {
        // A well-formed response whose leading block isn't text, so there's no
        // reply to hand back. (A content-free Message isn't representable —
        // the SDK builder rejects it.)
        let output = message_output(vec![ContentBlock::CachePoint(
            CachePointBlock::builder()
                .r#type(CachePointType::Default)
                .build()
                .expect("cache point builds"),
        )]);

        assert!(extract_text(Some(&output)).is_err());
    }

    #[tokio::test]
    async fn test_is_credentialed_is_false_without_a_provider() {
        let config = SdkConfig::builder().build();

        assert!(!is_credentialed(&config).await);
    }

    #[tokio::test]
    async fn test_is_credentialed_is_true_with_resolvable_credentials() {
        let config = SdkConfig::builder()
            .credentials_provider(SharedCredentialsProvider::new(Credentials::for_tests()))
            .build();

        assert!(is_credentialed(&config).await);
    }

    /// Stands in for the real credential chain on a machine with no AWS setup:
    /// a provider is present, but resolving it fails. This is the realistic
    /// case — `aws_config::defaults` always installs a chain, so the
    /// no-provider-at-all branch barely occurs in production.
    #[derive(Debug)]
    struct UnresolvableProvider;

    impl ProvideCredentials for UnresolvableProvider {
        fn provide_credentials<'a>(&'a self) -> provider::future::ProvideCredentials<'a>
        where
            Self: 'a,
        {
            provider::future::ProvideCredentials::ready(Err(CredentialsError::not_loaded(
                "no credentials in this environment",
            )))
        }
    }

    #[tokio::test]
    async fn test_is_credentialed_is_false_when_the_provider_fails() {
        let config = SdkConfig::builder()
            .credentials_provider(SharedCredentialsProvider::new(UnresolvableProvider))
            .build();

        assert!(!is_credentialed(&config).await);
    }

    #[tokio::test]
    async fn test_with_timeout_passes_through_fast_results() {
        let result = with_timeout(Duration::from_secs(30), async { Ok("done") })
            .await
            .unwrap();

        assert_eq!(result, "done");
    }

    #[tokio::test]
    async fn test_with_timeout_reports_the_budget_it_exceeded() {
        let result: Result<()> = with_timeout(Duration::from_millis(10), async {
            tokio::time::sleep(Duration::from_secs(30)).await;
            Ok(())
        })
        .await;

        let message = format!("{}", result.expect_err("should time out"));
        assert!(message.contains("10ms"), "{message}");
    }
}
