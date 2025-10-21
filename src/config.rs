use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::path::PathBuf;
use std::collections::HashMap;
use tokio::fs;
use crate::security::BashSecurity;
use log::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub servers: HashMap<String, McpServerConfig>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub max_tokens: u32,
    pub temperature: f32,
    pub default_system_prompt: Option<String>,
    pub bash_security: BashSecurity,
    pub mcp: McpConfig,
}
const DEFAULT_SYSTEM_PROMPT : &str = r#"
You are an expert in software development. Your job is to help the user build awesome software.

Everything you do must follow all best practices for architecture, design, security, and performance.

Whenever you generate code, you must make sure it compiles properly by running any available linter or compiler.

Generate a chain of thought, explaining your reasoning step-by-step before giving the final answer. Think deeply about what steps are required to proceed and tell me what they are.

When making tool calls, you must explain why you are making them, and what you hope to accomplish.
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
            bash_security: BashSecurity::default(),
            mcp: McpConfig::default(),
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

    /// Get the legacy mcp.toml path for migration
    pub fn legacy_mcp_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ai-agent")
            .join("mcp.toml")
    }

    /// Migrate MCP settings from legacy mcp.toml to unified config.toml
    pub async fn migrate_legacy_mcp_config() -> Result<()> {
        use crate::mcp::{McpConfig as LegacyMcpConfig};
        
        let legacy_path = Self::legacy_mcp_config_path();
        let unified_path = Self::default_config_path();
        
        if !legacy_path.exists() {
            info!("No legacy MCP config found at {}", legacy_path.display());
            return Ok(());
        }
        
        if unified_path.exists() {
            info!("Unified config already exists at {}, checking for MCP migration", unified_path.display());
            
            // Load existing unified config
            let existing_content = fs::read_to_string(&unified_path).await?;
            let unified_config: Config = toml::from_str(&existing_content)?;
            
            // If unified config already has MCP servers, skip migration
            if !unified_config.mcp.servers.is_empty() {
                info!("Unified config already contains MCP servers, skipping migration");
                return Ok(());
            }
        }
        
        // Load legacy MCP config
        info!("Migrating MCP settings from {} to {}", legacy_path.display(), unified_path.display());
        let legacy_content = fs::read_to_string(&legacy_path).await?;
        let legacy_config: LegacyMcpConfig = toml::from_str(&legacy_content)?;
        
        let server_count = legacy_config.servers.len();
        if server_count == 0 {
            info!("Legacy MCP config is empty, nothing to migrate");
            return Ok(());
        }
        
        // Load or create unified config
        let mut unified_config = if unified_path.exists() {
            let content = fs::read_to_string(&unified_path).await?;
            toml::from_str(&content)?
        } else {
            Config::default()
        };
        
        // Convert legacy server configs to new format
        for (name, legacy_server) in legacy_config.servers {
            let new_server = McpServerConfig {
                name: legacy_server.name,
                command: legacy_server.command,
                args: legacy_server.args,
                url: legacy_server.url,
                env: legacy_server.env,
                enabled: legacy_server.enabled,
            };
            unified_config.mcp.servers.insert(name, new_server);
        }
        
        // Save unified config
        unified_config.save(None).await?;
        
        // Backup and remove legacy config
        let backup_path = legacy_path.with_extension("toml.bak");
        fs::rename(&legacy_path, &backup_path).await?;
        
        info!("Successfully migrated {} MCP servers to unified config", server_count);
        info!("Legacy MCP config backed up to: {}", backup_path.display());
        
        Ok(())
    }

        /// Load configuration from file
      /// Load configuration from file and merge with environment variables
    pub async fn load(path: Option<&str>) -> Result<Self> {
        // First, try to migrate legacy MCP config
        if let Err(e) = Self::migrate_legacy_mcp_config().await {
            warn!("Failed to migrate legacy MCP config: {}", e);
        }
        
        let config_path = path
            .map(PathBuf::from)
            .unwrap_or_else(Self::default_config_path);

        let mut config = if config_path.exists() {
            let content = fs::read_to_string(&config_path).await?;
            let mut config: Config = toml::from_str(&content)?;
            
            // Ensure API key is never loaded from config file
            if !config.api_key.is_empty() {
                warn!("API key found in config file - ignoring for security. Use environment variables or command line.");
                config.api_key = String::new();
            }
            
            config
        } else {
            info!("No config file found at {}, using defaults", config_path.display());
            Config::default()
        };
        
        // Always prioritize environment variables for API key
        config.api_key = std::env::var("ANTHROPIC_AUTH_TOKEN").unwrap_or_default();
        
        Ok(config)
    }

    /// Save configuration to file (without API key)
    pub async fn save(&self, path: Option<&str>) -> Result<()> {
        let config_path = path
            .map(PathBuf::from)
            .unwrap_or_else(Self::default_config_path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Create a copy of the config without the API key for saving
        let mut config_for_save = self.clone();
        config_for_save.api_key = String::new(); // Clear API key before saving
        
        let content = toml::to_string_pretty(&config_for_save)?;
        fs::write(&config_path, content).await?;
        info!("Configuration saved to: {} (API key excluded for security)", config_path.display());
        Ok(())
    }

    /// Update MCP configuration and save to file
    pub async fn update_mcp_config(&mut self, mcp_config: McpConfig) -> Result<()> {
        self.mcp = mcp_config;
        self.save(None).await?;
        Ok(())
    }

    /// Update bash security configuration and save to file
    pub async fn update_bash_security(&mut self, bash_security: BashSecurity) -> Result<()> {
        self.bash_security = bash_security;
        self.save(None).await?;
        Ok(())
    }

    /// Create a sanitized copy of the config for saving (without API key)
    pub fn sanitized_for_save(&self) -> Self {
        let mut sanitized = self.clone();
        sanitized.api_key = String::new(); // Remove API key
        sanitized
    }
}