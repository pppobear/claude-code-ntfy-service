//! Configuration management handler
//!
//! This module handles all configuration-related commands including
//! initialization, setting values, and hook configuration.

use super::super::{CliContext, ConfigAction};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// Handler for configuration operations
pub struct ConfigHandler<'a> {
    context: &'a CliContext,
}

impl<'a> ConfigHandler<'a> {
    /// Create new configuration handler
    pub fn new(context: &'a CliContext) -> Self {
        Self { context }
    }

    /// Handle configuration initialization
    pub async fn handle_init(&self, global: bool, force: bool) -> Result<()> {
        let path = if global {
            None
        } else {
            self.context.project_path.clone().or_else(|| Some(PathBuf::from(".")))
        };

        // Check if config file already exists before creating ConfigManager
        let config_path = crate::config::ConfigManager::get_config_path(path.clone())?;
        let config_exists = config_path.exists();

        if config_exists && !force {
            println!("Configuration already initialized at: {}", config_path.display());
            println!("Use --force to overwrite");
            return Ok(());
        }

        // Create or load the configuration
        let config_manager = if global {
            crate::config::ConfigManager::new(None)?
        } else {
            // For project init, force creation of project config even if global exists
            crate::config::ConfigManager::new_project_config(path.unwrap_or_else(|| PathBuf::from(".")))?
        };

        // If config didn't exist or we're forcing, ensure it's saved
        if !config_exists || force {
            config_manager.save()?;
            println!("Configuration initialized successfully at: {}", config_path.display());
        }

        // Generate example hook scripts
        self.generate_hook_scripts()?;

        Ok(())
    }

    /// Handle configuration management
    pub async fn handle_config(&self, action: ConfigAction) -> Result<()> {
        // Create a mutable copy of the config manager for modifications
        let path = self.context.project_path.clone();
        let mut config_manager = crate::config::ConfigManager::new(path)?;

        match action {
            ConfigAction::Show => {
                let config = config_manager.config();
                println!("{}", toml::to_string_pretty(config)?);
            }
            ConfigAction::Set { key, value } => {
                // Simple key-value setter (can be expanded)
                match key.as_str() {
                    "ntfy.server_url" => config_manager.config_mut().ntfy.server_url = value.clone(),
                    "ntfy.default_topic" => {
                        config_manager.config_mut().ntfy.default_topic = value.clone()
                    }
                    "ntfy.auth_token" => {
                        config_manager.config_mut().ntfy.auth_token = Some(value.clone())
                    }
                    "daemon.enabled" => config_manager.config_mut().daemon.enabled = value.parse()?,
                    "daemon.log_path" => {
                        config_manager.config_mut().daemon.log_path = if value.is_empty() {
                            None
                        } else {
                            Some(value.clone())
                        }
                    }
                    "hooks.never_filter_decision_hooks" => {
                        config_manager.config_mut().hooks.never_filter_decision_hooks = value.parse()?
                    }
                    "hooks.decision_hook_priority" => {
                        let priority: u8 = value.parse().context("Priority must be a number 1-5")?;
                        if priority < 1 || priority > 5 {
                            return Err(anyhow::anyhow!("Priority must be between 1 and 5"));
                        }
                        config_manager.config_mut().hooks.decision_hook_priority = priority;
                    }
                    _ => return Err(anyhow::anyhow!("Unknown configuration key: {}", key)),
                }
                config_manager.save()?;
                println!("Configuration updated: {key} = {value}");
            }
            ConfigAction::Get { key } => {
                let value = match key.as_str() {
                    "ntfy.server_url" => config_manager.config().ntfy.server_url.clone(),
                    "ntfy.default_topic" => config_manager.config().ntfy.default_topic.clone(),
                    "daemon.enabled" => config_manager.config().daemon.enabled.to_string(),
                    "daemon.log_path" => config_manager.config().daemon.log_path
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| "None".to_string()),
                    "hooks.never_filter_decision_hooks" => {
                        config_manager.config().hooks.never_filter_decision_hooks.to_string()
                    }
                    "hooks.decision_hook_priority" => {
                        config_manager.config().hooks.decision_hook_priority.to_string()
                    }
                    _ => return Err(anyhow::anyhow!("Unknown configuration key: {}", key)),
                };
                println!("{value}");
            }
            ConfigAction::Hook {
                name,
                topic,
                priority,
                filter,
            } => {
                if let Some(topic) = topic {
                    config_manager
                        .config_mut()
                        .hooks
                        .topics
                        .insert(name.clone(), topic);
                }
                if let Some(priority) = priority {
                    config_manager
                        .config_mut()
                        .hooks
                        .priorities
                        .insert(name.clone(), priority);
                }
                if let Some(filter) = filter {
                    config_manager
                        .config_mut()
                        .hooks
                        .filters
                        .entry(name.clone())
                        .or_insert_with(Vec::new)
                        .push(filter);
                }
                config_manager.save()?;
                println!("Hook configuration updated for: {name}");
            }
        }

        Ok(())
    }

    /// Generate example hook scripts for the user
    fn generate_hook_scripts(&self) -> Result<()> {
        const CLAUDE_SETTINGS_TEMPLATE: &str = r#"
To use with Claude Code, add these to your settings:

.claude/settings.json or ~/.claude/settings.json:
{
  "hooks": {
    "PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "claude-ntfy"}]}],
    "PostToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "claude-ntfy"}]}],
    "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "claude-ntfy"}]}],
    "SessionStart": [{"hooks": [{"type": "command", "command": "claude-ntfy"}]}],
    "Stop": [{"hooks": [{"type": "command", "command": "claude-ntfy"}]}],
    "Notification": [{"hooks": [{"type": "command", "command": "claude-ntfy"}]}],
    "SubagentStop": [{"hooks": [{"type": "command", "command": "claude-ntfy"}]}]
  }
}

For project-specific configuration, run:
  claude-ntfy init --project .

To start the daemon:
  claude-ntfy daemon start      # Run in foreground (default)
  claude-ntfy daemon start -d   # Run in background (detached)
"#;

        print!("{}", CLAUDE_SETTINGS_TEMPLATE);
        Ok(())
    }
}

// Implement the handler factory trait to reduce boilerplate
super::traits::impl_context_handler!(ConfigHandler<'a>);
