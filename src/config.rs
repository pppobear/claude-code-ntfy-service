use crate::errors::{AppError, AppResult};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure for the Claude Code Ntfy Service
///
/// Contains all configuration sections including ntfy server settings,
/// hook processing configuration, template settings, and daemon options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ntfy: NtfyConfig,
    pub hooks: HookConfig,
    pub templates: TemplateConfig,
    pub daemon: DaemonConfig,
}

/// Configuration for ntfy notification service integration
///
/// Contains settings for connecting to and sending notifications via ntfy servers.
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

/// Configuration manager for the Claude Code Ntfy Service
///
/// Handles loading, saving, and managing configuration for both project-level
/// and global configurations. Project configurations take precedence over global ones.
///
/// # Configuration Hierarchy
/// 
/// 1. **Project-level**: `.claude/ntfy-service/config.toml` in project root
/// 2. **Global**: `~/.claude/ntfy-service/config.toml` in user home directory
///
/// # Example
///
/// ```rust,no_run
/// use claude_ntfy::config::ConfigManager;
/// use std::path::PathBuf;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Load project-specific configuration
///     let config_manager = ConfigManager::new(Some(PathBuf::from("/path/to/project")))?;
///     
///     // Access configuration
///     let ntfy_config = &config_manager.config().ntfy;
///     println!("Server URL: {}", ntfy_config.server_url);
///     Ok(())
/// }
/// ```
pub struct ConfigManager {
    #[allow(dead_code)]
    config_path: PathBuf,
    config: Config,
}

impl ConfigManager {
    /// Creates a new ConfigManager instance
    ///
    /// Loads configuration from the appropriate location based on the project path.
    /// If a project path is provided, looks for project-level configuration first,
    /// then falls back to global configuration if project config doesn't exist.
    /// If global config exists, uses it instead of creating a new project config.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Optional path to the project root. If `None`, uses global config.
    ///
    /// # Returns
    ///
    /// Returns a `ConfigManager` instance or an error if configuration loading fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The configuration directory cannot be created
    /// - The configuration file cannot be read or parsed
    /// - Default configuration cannot be serialized and written
    pub fn new(project_path: Option<PathBuf>) -> AppResult<Self> {
        if let Some(ref path) = project_path {
            let project_config_path = Self::get_config_path(Some(path.clone()))?;
            
            // If project config exists, use it
            if project_config_path.exists() {
                let config = Self::load_or_create(&project_config_path)?;
                return Ok(ConfigManager {
                    config_path: project_config_path,
                    config,
                });
            }
            
            // Check if global config exists
            let global_config_path = Self::get_config_path(None)?;
            if global_config_path.exists() {
                // Use global config instead of creating project config
                let config = Self::load_or_create(&global_config_path)?;
                return Ok(ConfigManager {
                    config_path: global_config_path,
                    config,
                });
            }
            
            // Neither exists, create project config
            let config = Self::load_or_create(&project_config_path)?;
            Ok(ConfigManager {
                config_path: project_config_path,
                config,
            })
        } else {
            // Global config requested
            let config_path = Self::get_config_path(None)?;
            let config = Self::load_or_create(&config_path)?;
            Ok(ConfigManager {
                config_path,
                config,
            })
        }
    }

    /// Creates a new ConfigManager instance with explicit project config creation
    ///
    /// Always creates or uses project-level configuration, even if global config exists.
    /// This method should be used when the user explicitly wants to create a project config.
    ///
    /// # Arguments
    ///
    /// * `project_path` - Path to the project root where config will be created
    ///
    /// # Returns
    ///
    /// Returns a `ConfigManager` instance with project-level configuration
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - The project path is None
    /// - The configuration directory cannot be created
    /// - The configuration file cannot be read or parsed
    /// - Default configuration cannot be serialized and written
    pub fn new_project_config(project_path: PathBuf) -> AppResult<Self> {
        let config_path = Self::get_config_path(Some(project_path))?;
        let config = Self::load_or_create(&config_path)?;

        Ok(ConfigManager {
            config_path,
            config,
        })
    }

    pub fn get_config_path(project_path: Option<PathBuf>) -> AppResult<PathBuf> {
        let base_path = if let Some(path) = project_path {
            // Project-level configuration
            path.join(".claude").join("ntfy-service")
        } else {
            // Global configuration
            let base_dirs = BaseDirs::new().ok_or_else(|| AppError::config("Failed to get base directories"))?;
            base_dirs.home_dir().join(".claude").join("ntfy-service")
        };

        // Ensure directory exists
        fs::create_dir_all(&base_path)
            .map_err(|e| AppError::io_with_source(&base_path, "create config directory", e))?;

        Ok(base_path.join("config.toml"))
    }

    fn load_or_create(path: &Path) -> AppResult<Config> {
        if path.exists() {
            let content = fs::read_to_string(path)
                .map_err(|e| AppError::io_with_source(path, "read config file", e))?;
            toml::from_str(&content)
                .map_err(|e| AppError::config_with_source("Failed to parse config file", e))
        } else {
            let config = Config::default();
            let content = toml::to_string_pretty(&config)
                .map_err(|e| AppError::config_with_source("Failed to serialize default config", e))?;
            fs::write(path, content)
                .map_err(|e| AppError::io_with_source(path, "write default config", e))?;
            Ok(config)
        }
    }

    /// Saves the current configuration to disk
    ///
    /// Writes the configuration back to the TOML file it was loaded from.
    /// This method is useful for persisting changes made to the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration cannot be serialized to TOML
    /// - The file cannot be written to disk
    #[allow(dead_code)]
    pub fn save(&self) -> AppResult<()> {
        let content = toml::to_string_pretty(&self.config)
            .map_err(|e| AppError::config_with_source("Failed to serialize config", e))?;
        fs::write(&self.config_path, content)
            .map_err(|e| AppError::io_with_source(&self.config_path, "write config file", e))?;
        Ok(())
    }

    /// Returns an immutable reference to the configuration
    ///
    /// Provides read-only access to the loaded configuration.
    /// Use this method to access configuration values without modifying them.
    ///
    /// # Returns
    ///
    /// A reference to the `Config` struct containing all configuration data.
    pub fn config(&self) -> &Config {
        &self.config
    }


    /// Returns a mutable reference to the configuration
    ///
    /// Provides write access to the configuration for making changes.
    /// After modifying the configuration, call [`save()`](Self::save) to persist changes.
    ///
    /// # Returns
    ///
    /// A mutable reference to the `Config` struct.
    #[allow(dead_code)]
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Reloads the configuration from disk
    ///
    /// Discards any in-memory changes and reloads the configuration from
    /// the original file path. This is useful for picking up external
    /// changes to the configuration file.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file cannot be read or parsed.
    #[allow(dead_code)]
    pub fn reload(&mut self) -> AppResult<()> {
        self.config = Self::load_or_create(&self.config_path)?;
        Ok(())
    }

    /// Gets the ntfy topic for a specific hook
    ///
    /// Returns the configured topic for the given hook name. If no specific
    /// topic is configured for the hook, returns the default topic.
    ///
    /// # Arguments
    ///
    /// * `hook_name` - The name of the hook (e.g., "PostToolUse", "PreToolUse")
    ///
    /// # Returns
    ///
    /// The topic string where notifications for this hook should be sent.
    pub fn get_hook_topic(&self, hook_name: &str) -> String {
        self.config
            .hooks
            .topics
            .get(hook_name)
            .cloned()
            .unwrap_or_else(|| self.config.ntfy.default_topic.clone())
    }

    /// Gets the notification priority for a specific hook
    ///
    /// Returns the configured priority for the given hook name. If no specific
    /// priority is configured for the hook, returns the default priority (3).
    ///
    /// # Arguments
    ///
    /// * `hook_name` - The name of the hook (e.g., "PostToolUse", "PreToolUse")
    ///
    /// # Returns
    ///
    /// Priority level (1-5, where 5 is highest priority).
    pub fn get_hook_priority(&self, hook_name: &str) -> u8 {
        self.config
            .hooks
            .priorities
            .get(hook_name)
            .cloned()
            .unwrap_or_else(|| self.config.ntfy.default_priority.unwrap_or(3))
    }

    /// Determines whether a hook should be processed based on configuration
    ///
    /// Checks if the hook is enabled and matches any configured filters.
    /// This method is used to decide whether to send notifications for a given hook.
    ///
    /// # Arguments
    ///
    /// * `hook_name` - The name of the hook to check
    /// * `hook_data` - The hook data to apply filters against
    ///
    /// # Returns
    ///
    /// `true` if the hook should be processed, `false` otherwise.
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
    pub fn get_effective_priority(&self, hook_name: &str, hook_data: &serde_json::Value) -> u8 {
        // If this is a decision-requiring hook, use the decision hook priority
        if self.is_decision_requiring_hook(hook_name, hook_data) {
            return self.config.hooks.decision_hook_priority;
        }
        
        // Otherwise use the configured priority for this hook type
        self.get_hook_priority(hook_name)
    }
}

// Removed unused Config::validate method

// Removed unused NtfyConfig::validate method

// Removed unused HookConfig::validate method

// Removed unused DaemonConfig::validate method
