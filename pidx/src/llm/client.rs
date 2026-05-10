use crate::config::LlmConfig;

use super::error::LlmError;
use super::types::{
    Classification, ClassifyRequest, ReduceArchitectureRequest,
    ReduceChangelogRequest, ReduceDescriptionRequest,
};
use super::{LlmClient, LlmFuture};

/// Stub adapter for any provider that speaks the Anthropic Messages
/// wire format (MiniMax, Doubao, Anthropic). Phase 0 only constructs
/// the struct and stores config; every trait method returns
/// [`LlmError::NotImplemented`].
pub struct AnthropicCompatibleClient {
    #[allow(dead_code)] // wired up in Phase 1
    base_url: String,
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    model: String,
    #[allow(dead_code)]
    http: reqwest::Client,
}

impl AnthropicCompatibleClient {
    /// Build a client from an [`LlmConfig`] block. The api key is read
    /// from `config.api_key_env` at construction time so a missing env
    /// var fails fast (rather than at the first LLM call).
    pub fn from_config(config: &LlmConfig) -> Result<Self, LlmError> {
        let api_key = std::env::var(&config.api_key_env).map_err(|_| {
            LlmError::Auth(format!(
                "env var {} not set (required by [llm].api_key_env)",
                config.api_key_env
            ))
        })?;

        let http = reqwest::Client::builder()
            .build()
            .map_err(LlmError::Http)?;

        Ok(Self {
            base_url: config.base_url.clone(),
            api_key,
            model: config.model.clone(),
            http,
        })
    }
}

impl LlmClient for AnthropicCompatibleClient {
    fn classify_commit<'a>(
        &'a self,
        _req: ClassifyRequest,
    ) -> LlmFuture<'a, Classification> {
        Box::pin(async { Err(LlmError::NotImplemented) })
    }

    fn reduce_changelog<'a>(
        &'a self,
        _req: ReduceChangelogRequest,
    ) -> LlmFuture<'a, String> {
        Box::pin(async { Err(LlmError::NotImplemented) })
    }

    fn reduce_architecture<'a>(
        &'a self,
        _req: ReduceArchitectureRequest,
    ) -> LlmFuture<'a, String> {
        Box::pin(async { Err(LlmError::NotImplemented) })
    }

    fn reduce_description<'a>(
        &'a self,
        _req: ReduceDescriptionRequest,
    ) -> LlmFuture<'a, String> {
        Box::pin(async { Err(LlmError::NotImplemented) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmBudgetConfig, LlmClassifyConfig};

    fn test_config(api_key_env: &str) -> LlmConfig {
        LlmConfig {
            provider: "minimax".to_string(),
            model: "MiniMax-M2".to_string(),
            api_key_env: api_key_env.to_string(),
            base_url: "https://example.invalid/anthropic".to_string(),
            max_concurrent_requests: 8,
            classify_max_tokens: 400,
            reduce_max_tokens: 2000,
            classify: LlmClassifyConfig::default(),
            budget: LlmBudgetConfig::default(),
        }
    }

    #[test]
    fn from_config_fails_when_api_key_env_missing() {
        // Arrange — pick a name guaranteed not to be set in this process.
        let env_name = "PIDX_TEST_LLM_KEY_DEFINITELY_UNSET_42";
        // SAFETY: tests are run with --test-threads=1 by default for env
        // mutation isn't safe here, so just make sure no one else is
        // using this exact name.
        unsafe {
            std::env::remove_var(env_name);
        }
        let config = test_config(env_name);

        // Act
        let result = AnthropicCompatibleClient::from_config(&config);

        // Assert
        match result {
            Err(LlmError::Auth(msg)) => assert!(msg.contains(env_name)),
            Err(other) => panic!("expected Auth error, got {other:?}"),
            Ok(_) => panic!("expected Auth error, got Ok(_)"),
        }
    }

    #[tokio::test]
    async fn classify_commit_returns_not_implemented() {
        // Arrange
        let env_name = "PIDX_TEST_LLM_KEY_PHASE0_STUB";
        // SAFETY: unique name per test, no other reader/writer races.
        unsafe {
            std::env::set_var(env_name, "dummy");
        }
        let config = test_config(env_name);
        let client = AnthropicCompatibleClient::from_config(&config).unwrap();
        let req = ClassifyRequest {
            repo_name: "taskq-rs".to_string(),
            sha: "deadbeef".to_string(),
            commit_subject: "test".to_string(),
            commit_body: String::new(),
            diff_excerpt: String::new(),
            prompt_version: 1,
        };

        // Act
        let result = client.classify_commit(req).await;

        // Assert
        assert!(matches!(result, Err(LlmError::NotImplemented)));
        // SAFETY: cleanup; same single-name guarantee as above.
        unsafe {
            std::env::remove_var(env_name);
        }
    }
}
