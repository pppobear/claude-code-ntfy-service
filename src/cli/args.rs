//! Argument processing and validation for CLI commands
//! 
//! This module handles argument validation, normalization, and processing
//! before commands are routed to their respective handlers.

use super::Commands;
use anyhow::Result;
use std::path::PathBuf;

/// Handles argument processing and validation
pub struct ArgProcessor;

impl ArgProcessor {
    /// Create a new argument processor
    pub fn new() -> Self {
        Self
    }

    /// Process the command and apply default values where needed
    pub fn process_command(&self, command: Option<Commands>) -> Result<Commands> {
        let command = command.unwrap_or_else(|| {
            // Default to Hook command when no subcommand is provided
            Commands::Hook {
                hook_name: None,
                no_daemon: false,
                dry_run: false,
            }
        });

        // Validate the processed command
        self.validate_command(&command)?;
        
        Ok(command)
    }

    /// Resolve the project path, using current directory as fallback if needed
    pub fn resolve_project_path(&self, project_path: Option<PathBuf>) -> Option<PathBuf> {
        project_path.or_else(|| {
            // For some commands, we need to provide a default path
            std::env::current_dir().ok()
        })
    }

    /// Validate command arguments
    fn validate_command(&self, command: &Commands) -> Result<()> {
        match command {
            Commands::Hook { .. } => {
                // Hook name validation will be handled in the handler since it can come from
                // multiple sources (arg, env, or JSON data)
                Ok(())
            }
            Commands::Test { priority, .. } => {
                if !(1..=5).contains(priority) {
                    return Err(anyhow::anyhow!(
                        "Priority must be between 1 and 5, got: {}", 
                        priority
                    ));
                }
                Ok(())
            }
            Commands::Config { action } => {
                self.validate_config_action(action)
            }
            Commands::Init { .. } | 
            Commands::Daemon { .. } | 
            Commands::Templates { .. } => {
                // No additional validation needed for these commands
                Ok(())
            }
        }
    }

    /// Validate configuration action arguments
    fn validate_config_action(&self, action: &super::ConfigAction) -> Result<()> {
        use super::ConfigAction;
        
        match action {
            ConfigAction::Set { key, value } => {
                if key.is_empty() {
                    return Err(anyhow::anyhow!("Configuration key cannot be empty"));
                }
                if value.is_empty() {
                    return Err(anyhow::anyhow!("Configuration value cannot be empty"));
                }
                
                // Validate known configuration keys
                self.validate_config_key(key)?;
                Ok(())
            }
            ConfigAction::Get { key } => {
                if key.is_empty() {
                    return Err(anyhow::anyhow!("Configuration key cannot be empty"));
                }
                self.validate_config_key(key)?;
                Ok(())
            }
            ConfigAction::Hook { name, priority, .. } => {
                if name.is_empty() {
                    return Err(anyhow::anyhow!("Hook name cannot be empty"));
                }
                
                if let Some(priority) = priority {
                    if !(1..=5).contains(priority) {
                        return Err(anyhow::anyhow!(
                            "Hook priority must be between 1 and 5, got: {}", 
                            priority
                        ));
                    }
                }
                Ok(())
            }
            ConfigAction::Show => Ok(()),
        }
    }

    /// Validate that a configuration key is known/supported
    fn validate_config_key(&self, key: &str) -> Result<()> {
        const VALID_CONFIG_KEYS: &[&str] = &[
            "ntfy.server_url",
            "ntfy.default_topic", 
            "ntfy.auth_token",
            "daemon.enabled",
            "daemon.log_path",
            "daemon.log_level",
            "daemon.max_queue_size",
            "daemon.retry_attempts",
            "daemon.retry_delay_secs",
            "hooks.enabled",
            "templates.use_custom",
        ];

        if !VALID_CONFIG_KEYS.contains(&key) {
            return Err(anyhow::anyhow!(
                "Unknown configuration key: {}. Valid keys are: {}", 
                key,
                VALID_CONFIG_KEYS.join(", ")
            ));
        }

        Ok(())
    }
}

impl Default for ArgProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Commands, ConfigAction, DaemonAction};

    #[test]
    fn test_process_command_default() {
        let processor = ArgProcessor::new();
        let result = processor.process_command(None).unwrap();
        
        match result {
            Commands::Hook { hook_name, no_daemon, dry_run } => {
                assert_eq!(hook_name, None);
                assert!(!no_daemon);
                assert!(!dry_run);
            }
            _ => panic!("Expected Hook command as default"),
        }
    }

    #[test]
    fn test_validate_test_command_priority() {
        let processor = ArgProcessor::new();
        
        // Valid priority
        let valid_cmd = Commands::Test {
            message: "test".to_string(),
            title: None,
            priority: 3,
            topic: None,
        };
        assert!(processor.validate_command(&valid_cmd).is_ok());
        
        // Invalid priority (too high)
        let invalid_cmd = Commands::Test {
            message: "test".to_string(),
            title: None,
            priority: 6,
            topic: None,
        };
        assert!(processor.validate_command(&invalid_cmd).is_err());
        
        // Invalid priority (too low)
        let invalid_cmd = Commands::Test {
            message: "test".to_string(),
            title: None,
            priority: 0,
            topic: None,
        };
        assert!(processor.validate_command(&invalid_cmd).is_err());
    }

    #[test]
    fn test_validate_config_key() {
        let processor = ArgProcessor::new();
        
        // Valid keys
        assert!(processor.validate_config_key("ntfy.server_url").is_ok());
        assert!(processor.validate_config_key("daemon.enabled").is_ok());
        
        // Invalid key
        assert!(processor.validate_config_key("invalid.key").is_err());
    }

    #[test]
    fn test_validate_config_action() {
        let processor = ArgProcessor::new();
        
        // Valid set action
        let valid_set = ConfigAction::Set {
            key: "ntfy.server_url".to_string(),
            value: "https://ntfy.sh".to_string(),
        };
        assert!(processor.validate_config_action(&valid_set).is_ok());
        
        // Invalid set action (empty key)
        let invalid_set = ConfigAction::Set {
            key: "".to_string(),
            value: "value".to_string(),
        };
        assert!(processor.validate_config_action(&invalid_set).is_err());
        
        // Invalid set action (unknown key)
        let invalid_key_set = ConfigAction::Set {
            key: "unknown.key".to_string(),
            value: "value".to_string(),
        };
        assert!(processor.validate_config_action(&invalid_key_set).is_err());
    }
}