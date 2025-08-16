//! Hook processing handler
//!
//! This module handles all hook-related commands including processing hook data,
//! sending to daemon, or processing directly.

use super::super::CliContext;
use crate::daemon::{NotificationTask, NtfyTaskConfig};
use crate::hooks::{self, DefaultHookProcessor, processor::HookProcessor};
use crate::ntfy::NtfyMessage;
use crate::shared::clients::create_sync_client_from_ntfy_config;
use crate::shared::ipc::convenience::send_notification_task;
use crate::shared::templates::{MessageFormatter, TemplateEngine, TemplateStyle};
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, Read};
use tracing::{debug, error, info};

/// Handler for hook processing operations
pub struct HookHandler<'a> {
    context: &'a CliContext,
    hook_processor: DefaultHookProcessor,
}

impl<'a> HookHandler<'a> {
    /// Create new hook handler
    pub fn new(context: &'a CliContext) -> Self {
        Self {
            context,
            hook_processor: hooks::create_default_processor(),
        }
    }

    /// Handle hook processing command
    pub async fn handle_hook(
        &self,
        hook_name: Option<String>,
        no_daemon: bool,
        dry_run: bool,
    ) -> Result<()> {
        // Read hook data from stdin (JSON) first
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read hook data from stdin")?;

        // First, try to get hook name from various sources
        let extracted_hook_name = if buffer.trim().is_empty() {
            // No stdin data - must get hook name from args or env
            hook_name.or_else(|| std::env::var("CLAUDE_HOOK").ok())
        } else {
            // Try to extract from JSON first, then fallback to args/env
            let parsed_data: Result<Value, _> = serde_json::from_str(&buffer);
            if let Ok(data) = &parsed_data {
                data.get("hook_event_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or(hook_name)
                    .or_else(|| std::env::var("CLAUDE_HOOK").ok())
            } else {
                hook_name.or_else(|| std::env::var("CLAUDE_HOOK").ok())
            }
        };

        let hook_name = extracted_hook_name
            .context("No hook name provided. Set CLAUDE_HOOK environment variable, use --hook-name, or ensure JSON contains 'hook_event_name' field")?;

        // Now parse hook data
        let raw_hook_data: Value = if buffer.trim().is_empty() {
            debug!("Processing hook from env: {}", hook_name);
            self.construct_hook_data_from_env(&hook_name)?
        } else {
            serde_json::from_str(&buffer).context("Failed to parse hook data as JSON")?
        };

        debug!("Processing hook: {}", hook_name);

        // Process hook using the new hooks module
        let processed_hook = self.hook_processor.process(&hook_name, raw_hook_data)
            .context("Failed to process hook with hooks module")?;
        
        let hook_data = processed_hook.enhanced_data.clone();
        debug!("Hook data (after enhancement): {:?}", hook_data);

        if dry_run {
            println!("Dry run - would send notification:");
            println!("Hook: {hook_name}");
            println!("Data: {}", serde_json::to_string_pretty(&hook_data)?);
            return Ok(());
        }

        // Use configuration from context
        let config_manager = &self.context.config_manager;

        // Check if hook should be processed
        if !config_manager.should_process_hook(&hook_name, &hook_data) {
            debug!("Hook {} filtered out, skipping", hook_name);
            return Ok(());
        }

        if !no_daemon && config_manager.config().daemon.enabled {
            // Send to daemon
            self.send_to_daemon(hook_name, hook_data).await?
        } else {
            // Process directly
            self.process_hook_directly(hook_name, hook_data)?
        }

        Ok(())
    }

    /// Construct hook data from environment variables
    fn construct_hook_data_from_env(&self, hook_name: &str) -> Result<Value> {
        let mut data = serde_json::json!({
            "hook": hook_name,
            "timestamp": chrono::Local::now().to_rfc3339(),
        });

        // Add common Claude Code environment variables
        if let Ok(tool) = std::env::var("CLAUDE_TOOL") {
            data["tool"] = Value::String(tool);
        }
        // Use CLAUDE_PROJECT_DIR (official)
        if let Ok(project_dir) = std::env::var("CLAUDE_PROJECT_DIR") {
            data["project_dir"] = Value::String(project_dir);
        }
        if let Ok(workspace) = std::env::var("CLAUDE_WORKSPACE") {
            data["workspace"] = Value::String(workspace);
        }

        // For tool hooks, add specific tool information
        if hook_name == "PreToolUse" || hook_name == "PostToolUse" {
            if let Ok(tool_name) = std::env::var("CLAUDE_TOOL_NAME") {
                data["tool_name"] = Value::String(tool_name);
            }
            
            // For PostToolUse, structure data to match expected schema
            if hook_name == "PostToolUse" {
                let mut tool_response = serde_json::Map::new();
                
                if let Ok(tool_status) = std::env::var("CLAUDE_TOOL_STATUS") {
                    tool_response.insert("success".to_string(), Value::Bool(tool_status == "success"));
                }
                
                data["tool_response"] = Value::Object(tool_response);
            } else if let Ok(tool_status) = std::env::var("CLAUDE_TOOL_STATUS") {
                // For PreToolUse, keep success at root level if needed
                data["success"] = Value::Bool(tool_status == "success");
            }
            
            if let Ok(duration) = std::env::var("CLAUDE_TOOL_DURATION") {
                data["duration_ms"] = Value::String(duration);
            }
        }

        Ok(data)
    }

    /// Send task to daemon via IPC
    async fn send_to_daemon(
        &self,
        hook_name: String,
        hook_data: Value,
    ) -> Result<()> {
        use crate::daemon::create_socket_path;
        
        // Use global socket path for daemon communication
        let socket_path = create_socket_path(None)?; // None = global socket
        
        // Check if daemon is running (simplified check for now)
        let pid_file = socket_path.with_extension("pid");
        if !pid_file.exists() {
            return Err(anyhow::anyhow!(
                "Global daemon is not running. Start it with 'claude-ntfy daemon start --global'"
            ));
        }

        // Get ntfy configuration from project config
        let config = self.context.config_manager.config();
        let topic = self.context.config_manager.get_hook_topic(&hook_name);
        let priority = self.context.config_manager.get_effective_priority(&hook_name, &hook_data);

        // Build ntfy task config from project settings
        let ntfy_config = NtfyTaskConfig {
            server_url: config.ntfy.server_url.clone(),
            topic,
            priority: Some(priority),
            tags: config.ntfy.default_tags.clone(),
            auth_token: config.ntfy.auth_token.clone(),
            send_format: config.ntfy.send_format.clone(),
        };

        let task = NotificationTask {
            hook_name,
            hook_data: serde_json::to_string(&hook_data)
                .context("Failed to serialize hook data")?,
            retry_count: 0,
            timestamp: chrono::Local::now(),
            ntfy_config,
            project_path: self.context.project_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        };

        // Send to daemon via IPC socket
        match send_notification_task(&socket_path, task).await {
            Ok(()) => {
                debug!("Hook task sent to global daemon successfully");
            }
            Err(e) => {
                error!("Failed to send hook task to global daemon: {}", e);
                return Err(e);
            }
        }

        debug!("Task sent to global daemon");
        Ok(())
    }


    /// Process hook directly without daemon
    fn process_hook_directly(
        &self,
        hook_name: String,
        hook_data: Value,
    ) -> Result<()> {
        let config_manager = &self.context.config_manager;
        let config = config_manager.config();

        // Create ntfy client using unified factory
        let client = create_sync_client_from_ntfy_config(&config.ntfy)?;

        // Create template engine and formatter
        let template_engine = TemplateEngine::new_with_style(TemplateStyle::Rich)?;
        let formatter = MessageFormatter::default();

        // Prepare message - use hook name directly (no transformation needed)
        let template_name = &hook_name;
        let formatted_data = template_engine.format_hook_data(&hook_name, &hook_data);

        let body = if config.templates.use_custom {
            if let Some(custom_template) = config.templates.custom_templates.get(&hook_name) {
                let mut hb = handlebars::Handlebars::new();
                hb.set_strict_mode(false);
                hb.render_template(custom_template, &formatted_data)
                    .unwrap_or_else(|e| {
                        error!("Failed to render custom template: {}", e);
                        template_engine
                            .render(
                                &template_name,
                                &formatted_data,
                            )
                            .unwrap_or_else(|_| format!("Hook: {hook_name}"))
                    })
            } else {
                template_engine.render(
                    &template_name,
                    &formatted_data,
                )?
            }
        } else {
            template_engine.render(
                &template_name,
                &formatted_data,
            )?
        };

        let title = formatter.format_title(&hook_name, &formatted_data);
        let topic = config_manager.get_hook_topic(&hook_name);
        let priority = config_manager.get_effective_priority(&hook_name, &hook_data);
        let mut tags = formatter.get_tags(&hook_name);
        if tags.is_empty() {
            tags = config.ntfy.default_tags.clone().unwrap_or_default();
        }

        let message = NtfyMessage {
            topic,
            title: Some(title),
            message: body,
            priority: Some(priority),
            tags: Some(tags),
            click: None,
            attach: None,
            filename: None,
            delay: None,
            email: None,
            call: None,
            actions: None,
        };

        // Send notification
        client
            .send(&message)
            .context("Failed to send notification")?;

        info!("Notification sent successfully for hook: {}", hook_name);
        Ok(())
    }
}

// Implement the handler factory trait to reduce boilerplate
super::traits::impl_context_handler!(HookHandler<'a>);
