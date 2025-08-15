//! Command handlers for all CLI operations
//! 
//! This module contains the implementation of all command handlers,
//! providing clean separation between CLI parsing and business logic.

use super::{Commands, ConfigAction, DaemonAction, CliContext};

// Import daemon types directly
use crate::daemon::{DaemonMessage, DaemonResponse, NotificationTask, NtfyTaskConfig, is_process_running, create_socket_path};

// Simple IPC client for daemon communication
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use crate::hooks::processor::HookProcessor;
use crate::hooks::{self, DefaultHookProcessor};
use crate::ntfy::NtfyMessage;
use crate::shared::clients::{create_sync_client_from_ntfy_config, create_async_client_from_ntfy_config};
use crate::templates::{MessageFormatter, TemplateEngine};
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;
use tracing::{debug, error, info, warn};


/// Coordinates all command handling operations with dependency injection via CliContext
pub struct CommandHandler {
    context: CliContext,
    hook_processor: DefaultHookProcessor,
}

impl CommandHandler {
    /// Create a new command handler instance with the provided context
    pub fn new(context: CliContext) -> Self {
        Self { 
            context,
            hook_processor: hooks::create_default_processor(),
        }
    }

    /// Route commands to their appropriate handlers
    pub async fn handle_command(&self, command: Commands) -> Result<()> {
        match command {
            Commands::Hook { hook_name, no_daemon, dry_run } => {
                self.handle_hook(hook_name, no_daemon, dry_run).await
            }
            Commands::Init { global, force } => {
                self.handle_init(global, force).await
            }
            Commands::Config { action } => {
                self.handle_config(action).await
            }
            Commands::Daemon { action } => {
                self.handle_daemon(action).await
            }
            Commands::Test { message, title, priority, topic } => {
                self.handle_test(message, title, priority, topic).await
            }
            Commands::Templates { show } => {
                self.handle_templates(show).await
            }
        }
    }

    /// Handle hook processing command
    async fn handle_hook(
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
            self.send_to_daemon(hook_name, hook_data).await?;
        } else {
            // Process directly
            self.process_hook_directly(hook_name, hook_data)?;
        }

        Ok(())
    }

    /// Handle configuration initialization
    async fn handle_init(&self, global: bool, force: bool) -> Result<()> {
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
    async fn handle_config(&self, action: ConfigAction) -> Result<()> {
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

    /// Handle daemon management operations
    async fn handle_daemon(&self, action: DaemonAction) -> Result<()> {
        match action {
            DaemonAction::Start { detach } => {
                self.handle_daemon_start(detach).await
            }
            DaemonAction::Stop => {
                self.handle_daemon_stop().await
            }
            DaemonAction::Status => {
                self.handle_daemon_status().await
            }
            DaemonAction::Reload => {
                self.handle_daemon_reload().await
            }
        }
    }

    /// Handle daemon start command
    async fn handle_daemon_start(&self, detach: bool) -> Result<()> {
        if detach {
            self.start_daemon_detached()
        } else {
            self.start_daemon_foreground().await
        }
    }

    /// Handle daemon stop command
    async fn handle_daemon_stop(&self) -> Result<()> {
        let (pid_file, _socket_path) = self.get_daemon_paths()?;

        match self.check_daemon_process(&pid_file)? {
            Some(pid_num) => {
                // Try to send shutdown signal via Unix socket IPC first
                match self.send_daemon_ipc_message(DaemonMessage::Shutdown).await {
                    Ok(_) => {
                        info!("Daemon stop signal sent via IPC");
                        
                        // Wait for daemon to stop (up to 10 seconds)
                        use std::time::{Duration, Instant};
                        let start_time = Instant::now();
                        let timeout = Duration::from_secs(10);

                        while start_time.elapsed() < timeout {
                            std::thread::sleep(Duration::from_millis(100));
                            if !is_process_running(pid_num) {
                                break;
                            }
                        }

                        // Verify process has stopped
                        if is_process_running(pid_num) {
                            println!("Warning: Daemon may still be running after stop signal");
                        } else {
                            println!("Daemon stopped successfully");
                            if pid_file.exists() {
                                let _ = std::fs::remove_file(&pid_file);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to send IPC shutdown signal: {}", e);
                        println!("Failed to send shutdown signal to daemon: {}", e);
                        
                        // Fallback: try to kill the process directly
                        #[cfg(unix)]
                        {
                            info!("Attempting to forcefully terminate daemon process...");
                            match std::process::Command::new("kill")
                                .arg("-TERM")
                                .arg(pid_num.to_string())
                                .status()
                            {
                                Ok(status) if status.success() => {
                                    info!("Sent SIGTERM to daemon process");
                                    
                                    // Wait a bit for graceful shutdown
                                    std::thread::sleep(std::time::Duration::from_secs(2));
                                    
                                    if is_process_running(pid_num) {
                                        warn!("Process still running, sending SIGKILL...");
                                        let _ = std::process::Command::new("kill")
                                            .arg("-KILL")
                                            .arg(pid_num.to_string())
                                            .status();
                                    }
                                    
                                    if pid_file.exists() {
                                        let _ = std::fs::remove_file(&pid_file);
                                    }
                                }
                                Ok(_) => error!("Failed to send signal to daemon process"),
                                Err(e) => error!("Failed to execute kill command: {}", e),
                            }
                        }
                        
                        #[cfg(not(unix))]
                        {
                            warn!("Cannot forcefully terminate daemon on this platform");
                        }
                    }
                }
            }
            None => println!("Daemon is not running"),
        }
        
        Ok(())
    }

    /// Handle daemon status command
    async fn handle_daemon_status(&self) -> Result<()> {
        let (pid_file, _) = self.get_daemon_paths()?;
        
        match self.check_daemon_process(&pid_file)? {
            Some(pid_num) => {
                // Try to get detailed status via IPC
                match self.send_daemon_ipc_message(DaemonMessage::Status).await {
                    Ok(DaemonResponse::Status { queue_size, is_running: _, uptime_secs }) => {
                        println!("Daemon is running (PID: {})", pid_num);
                        println!("  Queue size: {}", queue_size);
                        println!("  Uptime: {} seconds", uptime_secs);
                        println!("  IPC Status: Connected");
                    }
                    Ok(_) => {
                        println!("Daemon is running (PID: {}) - Unexpected status response", pid_num);
                    }
                    Err(e) => {
                        println!("Daemon is running (PID: {}) - IPC communication failed: {}", pid_num, e);
                    }
                }
            }
            None => println!("Daemon is not running"),
        }
        
        Ok(())
    }

    /// Handle daemon reload command
    async fn handle_daemon_reload(&self) -> Result<()> {
        let (pid_file, _) = self.get_daemon_paths()?;
        
        match self.check_daemon_process(&pid_file)? {
            Some(_pid_num) => {
                // Send reload signal via IPC
                match self.send_daemon_ipc_message(DaemonMessage::Reload).await {
                    Ok(DaemonResponse::Ok) => {
                        println!("Daemon reload signal sent successfully");
                    }
                    Ok(DaemonResponse::Error(e)) => {
                        println!("Daemon reload failed: {}", e);
                    }
                    Ok(_) => {
                        println!("Daemon reload - unexpected response");
                    }
                    Err(e) => {
                        println!("Failed to send reload signal to daemon: {}", e);
                    }
                }
            }
            None => {
                println!("Daemon is not running - cannot reload");
            }
        }
        
        Ok(())
    }

    /// Start daemon in detached (background) mode
    fn start_daemon_detached(&self) -> Result<()> {
        println!("Starting daemon in detached mode...");
        
        // Create socket path for daemon files
        let socket_path = create_socket_path(None)?;
        let pid_file = socket_path.with_extension("pid");
        
        // Check if daemon is already running
        match self.check_daemon_process(&pid_file)? {
            Some(pid_num) => {
                return Err(anyhow::anyhow!(
                    "Daemon is already running with PID: {}. Stop it first with 'claude-ntfy daemon stop'",
                    pid_num
                ));
            }
            None => {
                // No daemon running, safe to start new one
                debug!("No existing daemon found, proceeding with startup");
            }
        }
        
        // Ensure parent directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create socket directory")?;
        }

        // Get current executable path
        let current_exe = std::env::current_exe()
            .context("Failed to get current executable path")?;

        // Spawn a new process running the daemon in foreground mode
        // This avoids the tokio runtime nesting issue
        let mut child = process::Command::new(&current_exe)
            .arg("daemon")
            .arg("start")
            .env("CLAUDE_DAEMON_DETACHED", "1") // Signal to run detached
            .stdin(process::Stdio::null())
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .spawn()
            .context("Failed to spawn daemon process")?;

        // Wait briefly to see if the child process fails immediately
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        match child.try_wait()? {
            Some(exit_status) => {
                return Err(anyhow::anyhow!("Daemon process exited immediately: {}", exit_status));
            }
            None => {
                // Process is still running, consider it successfully started
                println!("Daemon started successfully with PID: {}", child.id());
            }
        }

        Ok(())
    }

    /// Start daemon in foreground mode
    async fn start_daemon_foreground(&self) -> Result<()> {
        // Check if we're running as a detached daemon
        let is_detached = std::env::var("CLAUDE_DAEMON_DETACHED").is_ok();
        
        // Create socket path for daemon files
        let socket_path = create_socket_path(None)?;
        let pid_file = socket_path.with_extension("pid");
        
        // Only check for existing daemon if this is NOT a detached process spawned by start_daemon_detached()
        // The detached process check was already done in the parent process
        if !is_detached {
            // Check if daemon is already running
            match self.check_daemon_process(&pid_file)? {
                Some(pid_num) => {
                    return Err(anyhow::anyhow!(
                        "Daemon is already running with PID: {}. Stop it first with 'claude-ntfy daemon stop'",
                        pid_num
                    ));
                }
                None => {
                    // No daemon running, safe to start new one
                    debug!("No existing daemon found, proceeding with startup");
                }
            }
        }
        
        if is_detached {
            // Detached mode - become session leader and close inherited handles
            println!("Detaching from parent process...");
            
            // Create new session (Unix only)
            #[cfg(unix)]
            unsafe {
                if libc::setsid() == -1 {
                    return Err(anyhow::anyhow!("Failed to create new session"));
                }
            }
        } else {
            println!("Starting daemon in foreground...");
        }
        
        // Write PID file for current process
        std::fs::write(&pid_file, process::id().to_string())
            .context("Failed to write PID file")?;
            
        info!("Daemon started with PID: {}", process::id());
        
        // Run integrated daemon in current async context
        self.run_integrated_daemon().await
    }

    /// Handle test notification
    async fn handle_test(
        &self,
        message: String,
        title: Option<String>,
        priority: u8,
        topic: Option<String>,
    ) -> Result<()> {
        let config_manager = &self.context.config_manager;
        let config = config_manager.config();

        let client = create_async_client_from_ntfy_config(&config.ntfy)?;

        let topic = topic.unwrap_or_else(|| config.ntfy.default_topic.clone());
        let title = title.unwrap_or_else(|| "Claude Ntfy Test".to_string());

        client.send_simple(&topic, &title, &message, priority).await?;

        println!("Test notification sent successfully");
        println!("Topic: {topic}");
        println!("Title: {title}");
        println!("Message: {message}");
        println!("Priority: {priority}");

        Ok(())
    }

    /// Handle template operations
    async fn handle_templates(&self, show: Option<String>) -> Result<()> {
        let template_engine = TemplateEngine::new()?;

        if let Some(template_name) = show {
            if let Some(content) = template_engine.get_template(&template_name) {
                println!("Template: {template_name}");
                println!("---");
                println!("{content}");
            } else {
                println!("Template '{template_name}' not found");
            }
        } else {
            println!("Available templates:");
            for template in template_engine.get_template_list() {
                println!("  - {template}");
            }
            println!("\nUse 'claude-ntfy templates --show <name>' to view a template");
        }

        Ok(())
    }
}

// Private helper methods
impl CommandHandler {
    /// Get daemon file paths (pid_file, socket_path)
    fn get_daemon_paths(&self) -> Result<(PathBuf, PathBuf)> {
        // Use global socket path for unified daemon
        let socket_path = create_socket_path(None)?; // None = global path
        let pid_file = socket_path.with_extension("pid");
        Ok((pid_file, socket_path))
    }

    /// Send a message to the daemon via Unix socket IPC
    async fn send_daemon_ipc_message(&self, message: DaemonMessage) -> Result<DaemonResponse> {
        let (_, socket_path) = self.get_daemon_paths()?;
        
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixStream;
        
        // Connect to Unix socket
        let mut stream = UnixStream::connect(&socket_path)
            .await
            .context("Failed to connect to daemon socket")?;

        // Serialize message
        let serialized = bincode::serialize(&message)
            .context("Failed to serialize message")?;

        let length = serialized.len() as u32;
        let length_bytes = length.to_le_bytes();

        // Send length prefix
        stream.write_all(&length_bytes).await
            .context("Failed to write message length")?;

        // Send message payload
        stream.write_all(&serialized).await
            .context("Failed to write message payload")?;

        stream.flush().await
            .context("Failed to flush message")?;

        // Read response length
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await
            .context("Failed to read response length")?;

        let response_length = u32::from_le_bytes(length_bytes) as usize;

        // Validate response length
        if response_length > 1024 * 1024 { // 1MB max response
            return Err(anyhow::anyhow!("Response too large: {} bytes", response_length));
        }

        // Read response payload
        let mut response_buffer = vec![0u8; response_length];
        stream.read_exact(&mut response_buffer).await
            .context("Failed to read response payload")?;

        // Deserialize response
        let response: DaemonResponse = bincode::deserialize(&response_buffer)
            .context("Failed to deserialize response")?;

        Ok(response)
    }

    /// Check daemon process status and clean up stale files
    fn check_daemon_process(&self, pid_file: &PathBuf) -> Result<Option<u32>> {
        if !pid_file.exists() {
            return Ok(None);
        }
        
        let pid_str = std::fs::read_to_string(pid_file)?;
        let pid = pid_str.trim();
        
        match pid.parse::<u32>() {
            Ok(pid_num) if is_process_running(pid_num) => Ok(Some(pid_num)),
            _ => {
                // Clean up stale/invalid PID file
                if let Err(e) = std::fs::remove_file(pid_file) {
                    warn!("Failed to remove stale PID file: {}", e);
                }
                Ok(None)
            }
        }
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
        match Self::send_task_to_daemon(&socket_path, task).await {
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

    /// Send a notification task to daemon via IPC socket
    async fn send_task_to_daemon(
        socket_path: &std::path::Path,
        task: NotificationTask,
    ) -> Result<()> {
        // Connect to Unix socket
        let mut stream = UnixStream::connect(socket_path)
            .await
            .context("Failed to connect to daemon socket")?;

        // Create message
        let message = DaemonMessage::Submit(task);

        // Serialize message
        let serialized = bincode::serialize(&message)
            .context("Failed to serialize message")?;

        let length = serialized.len() as u32;
        let length_bytes = length.to_le_bytes();

        // Send length prefix
        stream.write_all(&length_bytes).await
            .context("Failed to write message length")?;

        // Send message payload
        stream.write_all(&serialized).await
            .context("Failed to write message payload")?;

        stream.flush().await
            .context("Failed to flush message")?;

        // Read response length
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await
            .context("Failed to read response length")?;

        let response_length = u32::from_le_bytes(length_bytes) as usize;

        // Validate response length
        if response_length > 1024 { // 1KB max response
            return Err(anyhow::anyhow!("Response too large: {} bytes", response_length));
        }

        // Read response payload
        let mut response_buffer = vec![0u8; response_length];
        stream.read_exact(&mut response_buffer).await
            .context("Failed to read response payload")?;

        // Deserialize response
        let response: DaemonResponse = bincode::deserialize(&response_buffer)
            .context("Failed to deserialize response")?;

        match response {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(e) => Err(anyhow::anyhow!("Daemon error: {}", e)),
            _ => Err(anyhow::anyhow!("Unexpected response: {:?}", response)),
        }
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
        let template_engine = TemplateEngine::new()?;
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
                                Some(&config.templates.variables),
                            )
                            .unwrap_or_else(|_| format!("Hook: {hook_name}"))
                    })
            } else {
                template_engine.render(
                    &template_name,
                    &formatted_data,
                    Some(&config.templates.variables),
                )?
            }
        } else {
            template_engine.render(
                &template_name,
                &formatted_data,
                Some(&config.templates.variables),
            )?
        };

        let title = formatter.format_title(&hook_name, &formatted_data);
        let topic = config_manager.get_hook_topic(&hook_name);
        let priority = config_manager.get_effective_priority(&hook_name, &hook_data);
        let tags = formatter
            .get_tags(&hook_name)
            .or_else(|| config.ntfy.default_tags.clone());

        let message = NtfyMessage {
            topic,
            title: Some(title),
            message: body,
            priority: Some(priority),
            tags,
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

    /// Run integrated daemon with IPC server and notification processor
    async fn run_integrated_daemon(&self) -> Result<()> {
        use crate::daemon::{ipc_server::IpcServer, server::NotificationDaemon};
        use flume::unbounded;
        use std::sync::{atomic::AtomicUsize, Arc};

        // Create communication channels
        let (task_sender, task_receiver) = unbounded::<NotificationTask>();
        let (shutdown_sender, shutdown_receiver) = unbounded::<()>();
        let (ipc_shutdown_sender, ipc_shutdown_receiver) = unbounded::<()>();
        let (main_shutdown_sender, main_shutdown_receiver) = unbounded::<()>();
        let queue_size = Arc::new(AtomicUsize::new(0));

        // Create socket path
        let socket_path = create_socket_path(None)?; // Global daemon
        
        // Ensure parent directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create socket directory")?;
        }

        // Create IPC server
        let ipc_server = IpcServer::new(
            &socket_path,
            task_sender,
            ipc_shutdown_receiver,
            shutdown_sender.clone(),
            queue_size.clone(),
            main_shutdown_sender.clone(),
        )?;

        // Create notification daemon
        let notification_daemon = NotificationDaemon::new(
            task_receiver,
            shutdown_receiver,
            queue_size.clone(),
        )?;

        info!("Starting integrated daemon components");

        // Set up graceful shutdown on Ctrl+C
        let shutdown_sender_clone = shutdown_sender.clone();
        let ipc_shutdown_sender_clone = ipc_shutdown_sender.clone();
        let main_shutdown_sender_clone = main_shutdown_sender.clone();
        let socket_path_clone = socket_path.clone();
        let pid_file = socket_path.with_extension("pid");
        
        tokio::spawn(async move {
            if let Err(e) = tokio::signal::ctrl_c().await {
                error!("Failed to listen for Ctrl+C: {}", e);
                return;
            }
            
            info!("Received Ctrl+C signal, stopping daemon");
            
            // Send shutdown signals
            if let Err(e) = shutdown_sender_clone.send_async(()).await {
                warn!("Failed to send shutdown signal to notification daemon: {}", e);
            }
            
            if let Err(e) = ipc_shutdown_sender_clone.send_async(()).await {
                warn!("Failed to send shutdown signal to IPC server: {}", e);
            }
            
            // Signal main process to exit
            if let Err(e) = main_shutdown_sender_clone.send_async(()).await {
                warn!("Failed to send main shutdown signal: {}", e);
            }
        });

        // Clean up on exit
        let _guard = scopeguard::guard((), |_| {
            // Clean up socket and PID files
            if socket_path_clone.exists() {
                let _ = std::fs::remove_file(&socket_path_clone);
            }
            if pid_file.exists() {
                let _ = std::fs::remove_file(&pid_file);
            }
            info!("Daemon cleanup completed");
        });

        // Run IPC server and notification daemon concurrently, with shutdown handling
        tokio::select! {
            result = ipc_server.run() => {
                if let Err(e) = result {
                    error!("IPC server error: {}", e);
                }
            }
            result = notification_daemon.run() => {
                if let Err(e) = result {
                    error!("Notification daemon error: {}", e);
                }
            }
            result = main_shutdown_receiver.recv_async() => {
                match result {
                    Ok(_) => info!("Received main shutdown signal, terminating daemon"),
                    Err(e) => warn!("Main shutdown signal error: {}", e),
                }
            }
        }

        info!("Integrated daemon stopped");
        Ok(())
    }
}

// Note: Default trait removed as CommandHandler now requires CliContext