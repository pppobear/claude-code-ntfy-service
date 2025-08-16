use crate::errors::{AppError, AppResult};
use directories::BaseDirs;
use std::fs;
use std::path::{Path, PathBuf};

// Re-export shared types for convenience
pub use crate::shared::config::{Config, NtfyConfig};

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
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }


    /// Gets the ntfy topic for a specific hook
    pub fn get_hook_topic(&self, hook_name: &str) -> String {
        self.config
            .hooks
            .topics
            .get(hook_name)
            .cloned()
            .unwrap_or_else(|| self.config.ntfy.default_topic.clone())
    }

    /// Determines whether a hook should be processed based on configuration
    pub fn should_process_hook(&self, _hook_name: &str, _hook_data: &serde_json::Value) -> bool {
        self.config.hooks.enabled
    }
    
    /// Get effective priority for a hook, considering decision-requiring status
    pub fn get_effective_priority(&self, hook_name: &str, _hook_data: &serde_json::Value) -> u8 {
        self.config
            .hooks
            .priorities
            .get(hook_name)
            .cloned()
            .unwrap_or_else(|| self.config.ntfy.default_priority.unwrap_or(3))
    }

}

