use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub default_system_prompt: Option<String>,
}
const DEFAULT_SYSTEM_PROMPT : &str = r#"
You are an expert in software development. Your job is to help the user build awesome software.

Everything you do must follow all best practices for architecture, design, security, and performance.

Whenever you generate code, you must make sure it compiles properly by running any available linter or compiler.

Generate a chain of thought, explaining your reasoning step-by-step before giving the final answer.

When making tool calls, you must explain why you are making them.
"#;

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ANTHROPIC_AUTH_TOKEN").unwrap_or_default(),
            base_url: std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "https://api.anthropic.com/v1".to_string()),
            default_model: "glm-4.6".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            default_system_prompt: DEFAULT_SYSTEM_PROMPT.to_string().into(),
        }
    }
}

impl Config {
    pub fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ai-agent")
            .join("config.toml")
    }

    pub async fn load(path: Option<&str>) -> Result<Self> {
        let config_path = path
            .map(PathBuf::from)
            .unwrap_or_else(Self::default_config_path);

        if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub async fn save(&self, path: Option<&str>) -> Result<()> {
        let config_path = path
            .map(PathBuf::from)
            .unwrap_or_else(Self::default_config_path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content).await?;
        Ok(())
    }
}