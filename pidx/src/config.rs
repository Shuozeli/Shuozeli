use std::path::PathBuf;

use anyhow::{Context, bail};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub owner: String,
    pub index_path: Option<String>,
    pub sync: SyncConfig,
    pub repos: Vec<RepoEntry>,
    #[serde(default)]
    pub categories: Vec<CategoryEntry>,
    /// LLM doc pipeline configuration. Optional so existing pidx.toml
    /// files without an `[llm]` block keep loading; commands that need it
    /// (e.g. `pidx changelog`) fail-fast with a clear error when missing.
    #[serde(default)]
    pub llm: Option<LlmConfig>,
}

#[derive(Debug, Deserialize)]
pub struct SyncConfig {
    pub github_token_env: String,
    pub commits_per_sync: u32,
    pub db_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoEntry {
    pub name: String,
    pub category: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CategoryEntry {
    pub key: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // wired in Phase 1+; kept here so the toml schema is stable
pub struct LlmConfig {
    /// Provider key (e.g. "minimax", "doubao", "anthropic"). Used for
    /// telemetry / cache attribution; the wire format is the same for
    /// all three.
    pub provider: String,
    pub model: String,
    /// Name of the env var that holds the API key. The key itself is
    /// never read from the toml.
    pub api_key_env: String,
    pub base_url: String,
    #[serde(default = "LlmConfig::default_max_concurrent_requests")]
    pub max_concurrent_requests: u32,
    #[serde(default = "LlmConfig::default_classify_max_tokens")]
    pub classify_max_tokens: u32,
    #[serde(default = "LlmConfig::default_reduce_max_tokens")]
    pub reduce_max_tokens: u32,
    #[serde(default)]
    pub classify: LlmClassifyConfig,
    #[serde(default)]
    pub budget: LlmBudgetConfig,
}

impl LlmConfig {
    fn default_max_concurrent_requests() -> u32 {
        8
    }
    fn default_classify_max_tokens() -> u32 {
        400
    }
    fn default_reduce_max_tokens() -> u32 {
        2000
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // wired in Phase 1+
pub struct LlmClassifyConfig {
    /// Cap per-file diff size (in lines) when assembling the classify
    /// input. Keeps token usage bounded on large refactor commits.
    #[serde(default = "LlmClassifyConfig::default_diff_lines_per_file")]
    pub diff_lines_per_file: u32,
}

impl Default for LlmClassifyConfig {
    fn default() -> Self {
        Self {
            diff_lines_per_file: Self::default_diff_lines_per_file(),
        }
    }
}

impl LlmClassifyConfig {
    fn default_diff_lines_per_file() -> u32 {
        40
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // wired in Phase 5 (budget enforcement)
pub struct LlmBudgetConfig {
    /// Hard ceiling on total tokens (input + output) per UTC day.
    /// `None` means unlimited.
    #[serde(default)]
    pub daily_token_limit: Option<u64>,
    /// Emit a warning when usage crosses this percent of the daily
    /// limit (1-100). Ignored when `daily_token_limit` is unset.
    #[serde(default = "LlmBudgetConfig::default_warn_at_pct")]
    pub warn_at_pct: u8,
}

impl Default for LlmBudgetConfig {
    fn default() -> Self {
        Self {
            daily_token_limit: None,
            warn_at_pct: Self::default_warn_at_pct(),
        }
    }
}

impl LlmBudgetConfig {
    fn default_warn_at_pct() -> u8 {
        80
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            bail!(
                "Config file not found at {}. Create it manually.",
                path.display()
            );
        }
        let content =
            std::fs::read_to_string(&path).context("Failed to read config file")?;
        let config: Config =
            toml::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }

    pub fn config_path() -> PathBuf {
        Self::pidx_dir().join("pidx.toml")
    }

    pub fn pidx_dir() -> PathBuf {
        home_dir().join(".pidx")
    }

    pub fn db_path(&self) -> PathBuf {
        expand_tilde(&self.sync.db_path)
    }

    pub fn docs_dir() -> PathBuf {
        Self::pidx_dir().join("docs")
    }

    pub fn repo_docs_dir(repo_name: &str) -> PathBuf {
        Self::docs_dir().join(repo_name)
    }

    pub fn index_path(&self) -> anyhow::Result<PathBuf> {
        match &self.index_path {
            Some(p) => Ok(expand_tilde(p)),
            None => bail!("index_path not set in config"),
        }
    }

    pub fn github_token(&self) -> anyhow::Result<String> {
        let var_name = &self.sync.github_token_env;
        std::env::var(var_name)
            .with_context(|| format!("Environment variable {var_name} not set"))
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .expect("HOME environment variable not set")
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home_dir().join(rest)
    } else {
        PathBuf::from(path)
    }
}
