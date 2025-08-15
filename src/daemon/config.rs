use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ntfy: NtfyConfig,
    pub hooks: HookConfig,
    pub templates: TemplateConfig,
    pub daemon: DaemonConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NtfyConfig {
    pub server_url: String,
    pub default_topic: String,
    pub default_priority: Option<u8>,
    pub default_tags: Option<Vec<String>>,
    pub auth_token: Option<String>,
    pub timeout_secs: Option<u64>,
    pub send_format: String, // "text" or "json"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub enabled: bool,
    pub topics: HashMap<String, String>, // hook_name -> topic
    pub priorities: HashMap<String, u8>, // hook_name -> priority
    pub filters: HashMap<String, Vec<String>>, // hook_name -> filters
    pub never_filter_decision_hooks: bool, // Always allow decision-requiring hooks
    pub decision_hook_priority: u8, // Priority for hooks that require user decisions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub use_custom: bool,
    pub custom_templates: HashMap<String, String>, // hook_name -> template
    pub variables: HashMap<String, String>,        // custom variables
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub enabled: bool,
    pub socket_path: Option<PathBuf>,
    pub log_level: String,
    pub log_path: Option<String>,
    pub max_queue_size: usize,
    pub retry_attempts: u32,
    pub retry_delay_secs: u64,
}

impl Config {
    /// Default hook topics for different hook types
    fn default_hook_topics() -> HashMap<String, String> {
        let mut topics = HashMap::new();
        topics.insert("Notification".to_string(), "claude-decisions".to_string());
        topics.insert("PreToolUse".to_string(), "claude-tools".to_string());
        topics.insert("UserPromptSubmit".to_string(), "claude-prompts".to_string());
        topics
    }
    
    /// Default hook priorities - higher values = higher priority (1-5 scale)
    fn default_hook_priorities() -> HashMap<String, u8> {
        let mut priorities = HashMap::new();
        
        // Decision-requiring hooks get max priority (5)
        priorities.insert("Notification".to_string(), 5);
        priorities.insert("PreToolUse".to_string(), 4); // May require decisions
        priorities.insert("UserPromptSubmit".to_string(), 4); // May block prompts
        
        // Other hooks get lower priority
        priorities.insert("PostToolUse".to_string(), 3);
        priorities.insert("SessionStart".to_string(), 2);
        priorities.insert("Stop".to_string(), 2);
        priorities.insert("SubagentStop".to_string(), 2);
        priorities.insert("PreCompact".to_string(), 1);
        
        priorities
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ntfy: NtfyConfig {
                server_url: "https://ntfy.sh".to_string(),
                default_topic: "claude-code-hooks".to_string(),
                default_priority: Some(3),
                default_tags: Some(vec!["claude-code".to_string()]),
                auth_token: None,
                timeout_secs: Some(30),
                send_format: "text".to_string(), // Default to text for better compatibility
            },
            hooks: HookConfig {
                enabled: true,
                topics: Self::default_hook_topics(),
                priorities: Self::default_hook_priorities(),
                filters: HashMap::new(),
                never_filter_decision_hooks: true,
                decision_hook_priority: 5, // Max priority for decision hooks
            },
            templates: TemplateConfig {
                use_custom: false,
                custom_templates: HashMap::new(),
                variables: HashMap::new(),
            },
            daemon: DaemonConfig {
                enabled: true,
                socket_path: None,
                log_level: "info".to_string(),
                log_path: None, // Default to None, will use console logging
                max_queue_size: 1000,
                retry_attempts: 3,
                retry_delay_secs: 5,
            },
        }
    }
}

pub struct ConfigManager {
    #[allow(dead_code)]
    config_path: PathBuf,
    config: Config,
}

impl ConfigManager {
    #[allow(dead_code)]
    pub fn new(project_path: Option<PathBuf>) -> Result<Self> {
        let config_path = Self::get_config_path(project_path)?;
        let config = Self::load_or_create(&config_path)?;

        Ok(ConfigManager {
            config_path,
            config,
        })
    }

    #[allow(dead_code)]
    fn get_config_path(project_path: Option<PathBuf>) -> Result<PathBuf> {
        let base_path = if let Some(path) = project_path {
            // Project-level configuration
            path.join(".claude").join("ntfy-service")
        } else {
            // Global configuration
            let base_dirs = BaseDirs::new().context("Failed to get base directories")?;
            base_dirs.home_dir().join(".claude").join("ntfy-service")
        };

        // Ensure directory exists
        fs::create_dir_all(&base_path).context("Failed to create config directory")?;

        Ok(base_path.join("config.toml"))
    }

    fn load_or_create(path: &Path) -> Result<Config> {
        if path.exists() {
            let content = fs::read_to_string(path).context("Failed to read config file")?;
            toml::from_str(&content).context("Failed to parse config file")
        } else {
            let config = Config::default();
            let content =
                toml::to_string_pretty(&config).context("Failed to serialize default config")?;
            fs::write(path, content).context("Failed to write default config")?;
            Ok(config)
        }
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(&self.config).context("Failed to serialize config")?;
        fs::write(&self.config_path, content).context("Failed to write config file")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[allow(dead_code)]
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    #[allow(dead_code)]
    pub fn reload(&mut self) -> Result<()> {
        self.config = Self::load_or_create(&self.config_path)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_hook_topic(&self, hook_name: &str) -> String {
        self.config
            .hooks
            .topics
            .get(hook_name)
            .cloned()
            .unwrap_or_else(|| self.config.ntfy.default_topic.clone())
    }

    #[allow(dead_code)]
    pub fn get_hook_priority(&self, hook_name: &str) -> u8 {
        self.config
            .hooks
            .priorities
            .get(hook_name)
            .cloned()
            .unwrap_or_else(|| self.config.ntfy.default_priority.unwrap_or(3))
    }

    #[allow(dead_code)]
    pub fn should_process_hook(&self, hook_name: &str, hook_data: &serde_json::Value) -> bool {
        if !self.config.hooks.enabled {
            return false;
        }

        // Never filter decision-requiring hooks if the setting is enabled
        if self.config.hooks.never_filter_decision_hooks && self.is_decision_requiring_hook(hook_name, hook_data) {
            return true;
        }

        if let Some(filters) = self.config.hooks.filters.get(hook_name) {
            // Simple filter implementation - can be extended
            for filter in filters {
                if let Some(pattern) = filter.strip_prefix("!") {
                    // Exclusion filter
                    if hook_data.to_string().contains(pattern) {
                        return false;
                    }
                } else {
                    // Inclusion filter
                    if !hook_data.to_string().contains(filter) {
                        return false;
                    }
                }
            }
        }

        true
    }
    
    /// Check if a hook requires user decisions based on JSON decision control fields
    #[allow(dead_code)]
    pub fn is_decision_requiring_hook(&self, hook_name: &str, hook_data: &serde_json::Value) -> bool {
        match hook_name {
            // Notification hooks always require user attention
            "Notification" => true,
            
            // PreToolUse hooks require decisions if they have permission decision fields
            "PreToolUse" => {
                // Check for hookSpecificOutput.permissionDecision field
                if let Some(hook_specific) = hook_data.get("hookSpecificOutput") {
                    if let Some(permission_decision) = hook_specific.get("permissionDecision") {
                        if let Some(decision) = permission_decision.as_str() {
                            return matches!(decision, "allow" | "deny" | "ask");
                        }
                    }
                }
                
                // Check for deprecated decision field
                if let Some(decision) = hook_data.get("decision") {
                    if let Some(decision_str) = decision.as_str() {
                        return matches!(decision_str, "approve" | "block");
                    }
                }
                
                false
            },
            
            // PostToolUse hooks require decisions if they have blocking decision
            "PostToolUse" => {
                if let Some(decision) = hook_data.get("decision") {
                    if let Some(decision_str) = decision.as_str() {
                        return decision_str == "block";
                    }
                }
                false
            },
            
            // UserPromptSubmit hooks require decisions if they block prompts
            "UserPromptSubmit" => {
                if let Some(decision) = hook_data.get("decision") {
                    if let Some(decision_str) = decision.as_str() {
                        return decision_str == "block";
                    }
                }
                false
            },
            
            // Stop and SubagentStop hooks require decisions if they block stopping
            "Stop" | "SubagentStop" => {
                if let Some(decision) = hook_data.get("decision") {
                    if let Some(decision_str) = decision.as_str() {
                        return decision_str == "block";
                    }
                }
                false
            },
            
            _ => false
        }
    }
    
    /// Get effective priority for a hook, considering decision-requiring status
    #[allow(dead_code)]
    pub fn get_effective_priority(&self, hook_name: &str, hook_data: &serde_json::Value) -> u8 {
        // If this is a decision-requiring hook, use the decision hook priority
        if self.is_decision_requiring_hook(hook_name, hook_data) {
            return self.config.hooks.decision_hook_priority;
        }
        
        // Otherwise use the configured priority for this hook type
        self.get_hook_priority(hook_name)
    }
}
