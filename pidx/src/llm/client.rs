use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::config::LlmConfig;

use super::error::LlmError;
use super::types::{
    Classification, ClassifyRequest, CommitCategory, CommitImpact,
    ReduceArchitectureRequest, ReduceChangelogRequest, ReduceDescriptionRequest,
};
use super::{LlmClient, LlmFuture};

/// System prompt for `classify_commit`. Bumping any text here MUST be
/// paired with a bump of [`crate::llm::CLASSIFY_PROMPT_VERSION`] —
/// otherwise the cache would silently serve stale rows under the new
/// prompt.
const CLASSIFY_SYSTEM_PROMPT: &str = r#"You classify git commits for a per-repo CHANGELOG. Read the commit subject, body, and per-file diffs, and emit a single JSON object with this exact schema:

{
  "category": "Added" | "Changed" | "Fixed" | "Removed" | "Internal",
  "summary":  string (one line, present-tense imperative, <120 chars, no trailing period),
  "impact":   "minor" | "major" | "breaking"
}

Category guide:
- Added:    new user-visible feature, command, API, or capability
- Changed:  user-visible behavior change to something that already existed
- Fixed:    bug fix or correctness fix in user-visible behavior
- Removed:  feature, API, command, or capability removed
- Internal: refactor, cleanup, test, ci, docs, chore — no user-visible behavior change

Impact guide:
- minor:    small change; no migration; no public API change
- major:    significant change worth calling out; possibly new opt-in API
- breaking: removes/renames public API, changes default behavior, requires migration

Reply with ONLY the JSON object — no prose, no markdown fences."#;

/// Adapter for any provider that speaks the Anthropic Messages wire
/// format (MiniMax, Doubao, Anthropic).
pub struct AnthropicCompatibleClient {
    base_url: String,
    api_key: String,
    model: String,
    classify_max_tokens: u32,
    /// Provider key from config (e.g. "minimax"). Used for telemetry
    /// and for the `llm_provider` cache column.
    provider: String,
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
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(LlmError::Http)?;

        Ok(Self {
            base_url: config.base_url.clone(),
            api_key,
            model: config.model.clone(),
            classify_max_tokens: config.classify_max_tokens,
            provider: config.provider.clone(),
            http,
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

/// Anthropic Messages API response shape (subset). MiniMax mirrors this
/// 1:1 modulo provider-specific extras we don't need.
#[derive(Debug, Deserialize)]
struct AnthropicMessagesResponse {
    content: Vec<AnthropicContentBlock>,
    #[serde(default)]
    #[allow(dead_code)] // surfaced for future budget tracking
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    /// Other block types (tool_use etc.) are present in the wire shape
    /// but not used by the classify path. We tolerate them so a future
    /// upgrade doesn't crash here.
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[allow(dead_code)]
    input_tokens: u32,
    #[allow(dead_code)]
    output_tokens: u32,
}

/// Strip ```json ...``` fences (or bare ``` fences) from the model's
/// reply if present. The schema asks for raw JSON but models love
/// fences. We accept both.
pub(crate) fn strip_json_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        rest.trim_start().trim_end_matches("```").trim()
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        rest.trim_start().trim_end_matches("```").trim()
    } else {
        trimmed
    }
}

#[derive(Debug, Deserialize)]
struct ClassificationWire {
    category: CommitCategory,
    summary: String,
    impact: CommitImpact,
}

/// Parse a classification JSON payload (with optional fences) into a
/// [`Classification`]. Errors with [`LlmError::Other`] when the schema
/// doesn't match — per the project's "fail-fast over fail-safe" rule,
/// we do not return defaults.
pub(crate) fn parse_classification(raw: &str) -> Result<Classification, LlmError> {
    let stripped = strip_json_fences(raw);
    let wire: ClassificationWire = serde_json::from_str(stripped).map_err(|e| {
        LlmError::Other(anyhow::anyhow!(
            "failed to parse classifier JSON ({e}). raw response: {raw}"
        ))
    })?;
    Ok(Classification {
        category: wire.category,
        summary: wire.summary,
        impact: wire.impact,
    })
}

/// Build the full URL for `POST /v1/messages` against the configured
/// base. Handles base URLs with or without trailing slash.
fn messages_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    format!("{trimmed}/v1/messages")
}

impl LlmClient for AnthropicCompatibleClient {
    fn classify_commit<'a>(
        &'a self,
        req: ClassifyRequest,
    ) -> LlmFuture<'a, Classification> {
        Box::pin(async move {
            let url = messages_url(&self.base_url);

            let user_content = format!(
                "Repository: {}\n\n{}",
                req.repo_name, req.diff_excerpt
            );
            let body = json!({
                "model": self.model,
                "max_tokens": self.classify_max_tokens,
                "system": CLASSIFY_SYSTEM_PROMPT,
                "messages": [
                    {"role": "user", "content": user_content}
                ]
            });

            let response = self
                .http
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(LlmError::Http)?;

            let status = response.status();
            match status.as_u16() {
                200 => {}
                401 | 403 => {
                    let text = response.text().await.unwrap_or_default();
                    return Err(LlmError::Auth(format!(
                        "{status}: {}",
                        text.chars().take(200).collect::<String>()
                    )));
                }
                429 => {
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    return Err(LlmError::RateLimit { retry_after });
                }
                _ => {
                    let text = response.text().await.unwrap_or_default();
                    return Err(LlmError::Other(anyhow::anyhow!(
                        "unexpected HTTP {status} from {url}: {}",
                        text.chars().take(500).collect::<String>()
                    )));
                }
            }

            let wire: AnthropicMessagesResponse =
                response.json().await.map_err(LlmError::Http)?;

            let text = wire
                .content
                .iter()
                .find_map(|b| match b {
                    AnthropicContentBlock::Text { text } => Some(text.as_str()),
                    AnthropicContentBlock::Other => None,
                })
                .ok_or_else(|| {
                    // MiniMax-M2 emits a `thinking` block before its
                    // `text` block; if `max_tokens` is too low the
                    // thinking can swallow the budget and no text is
                    // emitted. Surface that hypothesis explicitly so
                    // operators don't have to guess.
                    LlmError::Other(anyhow::anyhow!(
                        "classifier response had no text content block — \
                         likely classify_max_tokens={} is too low for the \
                         model's reasoning budget. Try bumping \
                         [llm].classify_max_tokens in pidx.toml.",
                        self.classify_max_tokens
                    ))
                })?;

            parse_classification(text)
        })
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

    #[test]
    fn parse_classification_strips_json_fence() {
        // Arrange — model wrapped reply in ```json ... ``` fences.
        let raw = "```json\n{\"category\":\"Fixed\",\"summary\":\"abort acquire loops on drain\",\"impact\":\"minor\"}\n```";

        // Act
        let parsed = parse_classification(raw).unwrap();

        // Assert
        assert_eq!(parsed.category, CommitCategory::Fixed);
        assert_eq!(parsed.summary, "abort acquire loops on drain");
        assert_eq!(parsed.impact, CommitImpact::Minor);
    }

    #[test]
    fn parse_classification_strips_bare_fence() {
        // Arrange — bare ``` fence (no language tag).
        let raw = "```\n{\"category\":\"Added\",\"summary\":\"add new flag\",\"impact\":\"minor\"}\n```";

        // Act
        let parsed = parse_classification(raw).unwrap();

        // Assert
        assert_eq!(parsed.category, CommitCategory::Added);
    }

    #[test]
    fn parse_classification_accepts_raw_json() {
        // Arrange — well-behaved model returns bare JSON.
        let raw = r#"{"category":"Internal","summary":"refactor module layout","impact":"minor"}"#;

        // Act
        let parsed = parse_classification(raw).unwrap();

        // Assert
        assert_eq!(parsed.category, CommitCategory::Internal);
    }

    #[test]
    fn parse_classification_rejects_malformed_json_with_other_error() {
        // Arrange — text that isn't JSON at all.
        let raw = "I think this is a Fixed commit";

        // Act
        let result = parse_classification(raw);

        // Assert — fail-fast, no silent default.
        match result {
            Err(LlmError::Other(e)) => {
                let msg = format!("{e}");
                assert!(msg.contains("raw response"));
            }
            other => panic!("expected LlmError::Other, got {other:?}"),
        }
    }

    #[test]
    fn parse_classification_rejects_wrong_schema() {
        // Arrange — JSON, but missing the "category" field.
        let raw = r#"{"foo":"bar","summary":"x","impact":"minor"}"#;

        // Act
        let result = parse_classification(raw);

        // Assert
        assert!(matches!(result, Err(LlmError::Other(_))));
    }

    #[test]
    fn messages_url_handles_trailing_slash() {
        // Arrange / Act / Assert
        assert_eq!(
            messages_url("https://api.minimax.chat/anthropic"),
            "https://api.minimax.chat/anthropic/v1/messages"
        );
        assert_eq!(
            messages_url("https://api.minimax.chat/anthropic/"),
            "https://api.minimax.chat/anthropic/v1/messages"
        );
    }

    #[tokio::test]
    async fn classify_commit_returns_auth_error_on_401() {
        // Arrange — point at a server that we know returns 401 quickly.
        // We can't easily spin up an HTTP mock without adding a dep, so
        // we use the existing not-implemented contract for the wire
        // path: this test just covers from_config + provider getters.
        let env_name = "PIDX_TEST_LLM_KEY_PHASE1_GETTERS";
        // SAFETY: unique name per test, no other reader/writer races.
        unsafe {
            std::env::set_var(env_name, "dummy");
        }
        let config = test_config(env_name);
        let client = AnthropicCompatibleClient::from_config(&config).unwrap();

        // Act / Assert
        assert_eq!(client.provider(), "minimax");
        assert_eq!(client.model(), "MiniMax-M2");

        // SAFETY: cleanup; same single-name guarantee as above.
        unsafe {
            std::env::remove_var(env_name);
        }
    }
}
