use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;
use tracing::{debug, error, info};

mod config;
mod ntfy;
mod templates;

use config::ConfigManager;
use ntfy::{NtfyClient, NtfyMessage};
use templates::{MessageFormatter, TemplateEngine};

#[derive(Parser)]
#[command(name = "claude-ntfy")]
#[command(about = "Claude Code hook CLI tool with ntfy integration")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Project path for project-level configuration
    #[arg(long, global = true)]
    project: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Handle Claude Code hook (default mode when no subcommand)
    Hook {
        /// Hook name (from CLAUDE_HOOK environment variable)
        #[arg(short = 'n', long, env = "CLAUDE_HOOK")]
        hook_name: Option<String>,

        /// Don't send to daemon, process directly
        #[arg(long)]
        no_daemon: bool,

        /// Dry run - don't actually send notification
        #[arg(long)]
        dry_run: bool,
    },

    /// Initialize configuration
    Init {
        /// Initialize global configuration (default is project-level)
        #[arg(short, long)]
        global: bool,

        /// Force overwrite existing configuration
        #[arg(short, long)]
        force: bool,
    },

    /// Configure settings
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Daemon management
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Test notification sending
    Test {
        /// Message to send
        message: String,

        /// Optional title
        #[arg(short, long)]
        title: Option<String>,

        /// Priority (1-5)
        #[arg(short, long, default_value = "3")]
        priority: u8,

        /// Topic to send to
        #[arg(short = 'o', long)]
        topic: Option<String>,
    },

    /// List available templates
    Templates {
        /// Show template content
        #[arg(short, long)]
        show: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,

    /// Set configuration value
    Set {
        /// Configuration key (e.g., ntfy.server_url)
        key: String,
        /// Value to set
        value: String,
    },

    /// Get configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set hook-specific configuration
    Hook {
        /// Hook name
        name: String,
        /// Topic for this hook
        #[arg(long)]
        topic: Option<String>,
        /// Priority for this hook
        #[arg(long)]
        priority: Option<u8>,
        /// Add filter for this hook
        #[arg(long)]
        filter: Option<String>,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon
    Start {
        /// Run in detached mode (background)
        #[arg(short = 'd', long)]
        detach: bool,
    },

    /// Stop the daemon
    Stop,

    /// Check daemon status
    Status,

    /// Reload daemon configuration
    Reload,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(log_level.parse().unwrap()),
        )
        .init();

    // Handle default hook mode when called without subcommand
    let command = cli.command.unwrap_or({
        Commands::Hook {
            hook_name: None,
            no_daemon: false,
            dry_run: false,
        }
    });

    let result = match command {
        Commands::Hook {
            hook_name,
            no_daemon,
            dry_run,
        } => handle_hook(cli.project, hook_name, no_daemon, dry_run),
        Commands::Init { global, force } => init_config(cli.project, global, force),
        Commands::Config { action } => handle_config(cli.project, action),
        Commands::Daemon { action } => handle_daemon(cli.project, action),
        Commands::Test {
            message,
            title,
            priority,
            topic,
        } => test_notification(cli.project, message, title, priority, topic),
        Commands::Templates { show } => handle_templates(cli.project, show),
    };

    // Handle exit codes properly for Claude Code hooks
    match result {
        Ok(_) => std::process::exit(0), // Success
        Err(e) => {
            eprintln!("Error: {}", e);
            // Check if this is a blocking error that should return exit code 2
            if e.to_string().contains("block") || e.to_string().contains("Block") {
                std::process::exit(2); // Blocking error
            } else {
                std::process::exit(1); // Regular error
            }
        }
    }
}

fn enhance_hook_data(hook_name: &str, mut hook_data: Value) -> Value {
    // Enhance PostToolUse data to ensure proper success field handling
    if hook_name == "PostToolUse" {
        if let Value::Object(ref mut map) = hook_data {
            // Check if tool_response exists and has success field
            if let Some(tool_response) = map.get_mut("tool_response") {
                if let Value::Object(ref mut response_map) = tool_response {
                    // If success field is missing, try to infer it
                    if !response_map.contains_key("success") {
                        debug!("PostToolUse: tool_response.success field missing, inferring from other fields");
                        
                        // Infer success based on:
                        // 1. Absence of error field
                        // 2. Presence of filePath or other success indicators
                        let has_error = response_map.contains_key("error");
                        let has_file_path = response_map.contains_key("filePath");
                        let inferred_success = !has_error;
                        
                        response_map.insert("success".to_string(), Value::Bool(inferred_success));
                        
                        debug!("PostToolUse: Inferred success={} (has_error={}, has_file_path={})", 
                               inferred_success, has_error, has_file_path);
                    } else {
                        debug!("PostToolUse: tool_response.success field found: {:?}", 
                               response_map.get("success"));
                    }
                }
            } else {
                // If tool_response is missing entirely, create it
                debug!("PostToolUse: tool_response object missing, creating with inferred success");
                
                // Look for success indicators at the root level
                let root_success = map.get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true); // Default to true if no failure indicators
                
                let mut tool_response = serde_json::Map::new();
                tool_response.insert("success".to_string(), Value::Bool(root_success));
                
                map.insert("tool_response".to_string(), Value::Object(tool_response));
                
                debug!("PostToolUse: Created tool_response with success={}", root_success);
            }
        }
    }
    
    hook_data
}

fn handle_hook(
    project_path: Option<PathBuf>,
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
    let hook_data: Value = if buffer.trim().is_empty() {
        debug!("Processing hook from env: {}", hook_name);
        construct_hook_data_from_env(&hook_name)?
    } else {
        serde_json::from_str(&buffer).context("Failed to parse hook data as JSON")?
    };

    debug!("Processing hook: {}", hook_name);

    // Enhance hook data for better processing
    let hook_data = enhance_hook_data(&hook_name, hook_data);
    
    debug!("Hook data (after enhancement): {:?}", hook_data);

    if dry_run {
        println!("Dry run - would send notification:");
        println!("Hook: {hook_name}");
        println!("Data: {}", serde_json::to_string_pretty(&hook_data)?);
        return Ok(());
    }

    // Load configuration
    let config_manager = ConfigManager::new(project_path.clone())?;

    // Check if hook should be processed
    if !config_manager.should_process_hook(&hook_name, &hook_data) {
        debug!("Hook {} filtered out, skipping", hook_name);
        return Ok(());
    }

    if !no_daemon && config_manager.config().daemon.enabled {
        // Send to daemon
        send_to_daemon(project_path, hook_name, hook_data)?;
    } else {
        // Process directly
        process_hook_directly(config_manager, hook_name, hook_data)?;
    }

    Ok(())
}

fn construct_hook_data_from_env(hook_name: &str) -> Result<Value> {
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

fn send_to_daemon(
    project_path: Option<PathBuf>,
    hook_name: String,
    hook_data: Value,
) -> Result<()> {
    use std::fs;

    // Get socket path
    let socket_path = daemon::create_socket_path(project_path.as_ref())?;

    // Check if daemon is running
    let pid_file = socket_path.with_extension("pid");
    if !pid_file.exists() {
        return Err(anyhow::anyhow!(
            "Daemon is not running. Start it with 'claude-ntfy daemon start'"
        ));
    }

    // Create notification task
    let task = daemon::NotificationTask {
        hook_name,
        hook_data,
        retry_count: 0,
        timestamp: chrono::Local::now(),
    };

    // Send to daemon via command file (simplified IPC)
    let cmd_file = socket_path.with_extension("cmd");
    let message = daemon::DaemonMessage::Submit(task);
    let content = serde_json::to_string(&message)?;
    fs::write(&cmd_file, content).context("Failed to send task to daemon")?;

    debug!("Task sent to daemon");
    Ok(())
}

fn process_hook_directly(
    config_manager: ConfigManager,
    hook_name: String,
    hook_data: Value,
) -> Result<()> {
    let config = config_manager.config();

    // Create ntfy client
    let client = NtfyClient::new(
        config.ntfy.server_url.clone(),
        config.ntfy.auth_token.clone(),
        config.ntfy.timeout_secs,
        config.ntfy.send_format.clone(),
    )?;

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
    let priority = config_manager.get_hook_priority(&hook_name);
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

fn init_config(project_path: Option<PathBuf>, global: bool, force: bool) -> Result<()> {
    let path = if global {
        None
    } else {
        project_path.or_else(|| Some(PathBuf::from(".")))
    };

    let config_manager = ConfigManager::new(path)?;

    if !force {
        // Config already exists (created by ConfigManager::new)
        println!(
            "Configuration already initialized at: {:?}",
            config_manager.config()
        );
        println!("Use --force to overwrite");
        return Ok(());
    }

    // Save default configuration
    config_manager.save()?;
    println!("Configuration initialized successfully");

    // Generate example hook scripts
    generate_hook_scripts()?;

    Ok(())
}

fn generate_hook_scripts() -> Result<()> {
    println!("\nTo use with Claude Code, add these to your settings:");
    println!("\n.claude/settings.json or ~/.claude/settings.json:");
    println!(
        r#"{{
  "hooks": {{
    "PreToolUse": [
      {{
        "matcher": "*",
        "hooks": [
          {{
            "type": "command",
            "command": "claude-ntfy"
          }}
        ]
      }}
    ],
    "PostToolUse": [
      {{
        "matcher": "*",
        "hooks": [
          {{
            "type": "command",
            "command": "claude-ntfy"
          }}
        ]
      }}
    ],
    "UserPromptSubmit": [
      {{
        "hooks": [
          {{
            "type": "command",
            "command": "claude-ntfy"
          }}
        ]
      }}
    ],
    "SessionStart": [
      {{
        "hooks": [
          {{
            "type": "command",
            "command": "claude-ntfy"
          }}
        ]
      }}
    ],
    "Stop": [
      {{
        "hooks": [
          {{
            "type": "command",
            "command": "claude-ntfy"
          }}
        ]
      }}
    ],
    "Notification": [
      {{
        "hooks": [
          {{
            "type": "command",
            "command": "claude-ntfy"
          }}
        ]
      }}
    ],
    "SubagentStop": [
      {{
        "hooks": [
          {{
            "type": "command",
            "command": "claude-ntfy"
          }}
        ]
      }}
    ]
  }}
}}"#
    );

    println!("\nFor project-specific configuration, run:");
    println!("  claude-ntfy init --project .");

    println!("\nTo start the daemon:");
    println!("  claude-ntfy daemon start      # Run in foreground (default)");
    println!("  claude-ntfy daemon start -d   # Run in background (detached)");

    Ok(())
}

fn handle_config(project_path: Option<PathBuf>, action: ConfigAction) -> Result<()> {
    let mut config_manager = ConfigManager::new(project_path)?;

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

fn handle_daemon(project_path: Option<PathBuf>, action: DaemonAction) -> Result<()> {
    match action {
        DaemonAction::Start { detach } => {
            // Use current directory as default project path to ensure consistency
            let daemon_project_path = project_path
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

            if detach {
                // Start daemon in background (detached)
                let daemon_binary = if cfg!(debug_assertions) {
                    "./target/debug/claude-ntfy-daemon"
                } else {
                    "claude-ntfy-daemon"
                };
                let mut child = process::Command::new(daemon_binary)
                    .arg("--background")
                    .arg(daemon_project_path.to_string_lossy().to_string())
                    .stdout(process::Stdio::null())  // 重定向标准输出到 null
                    .stderr(process::Stdio::piped())  // 捕获标准错误以检查启动错误
                    .spawn()?;
                
                // Wait a short time for the daemon to start and perform initial checks
                use std::time::{Duration, Instant};
                let start_time = Instant::now();
                let timeout = Duration::from_secs(3);
                
                while start_time.elapsed() < timeout {
                    // Check if the child process has exited (indicating an error during startup)
                    match child.try_wait()? {
                        Some(exit_status) => {
                            // Process exited, which means there was an error
                            let output = child.wait_with_output()?;
                            let error_msg = String::from_utf8_lossy(&output.stderr);
                            return Err(anyhow::anyhow!("Failed to start daemon: {}", error_msg.trim()));
                        }
                        None => {
                            // Process is still running, check if PID file has been created
                            let socket_path = daemon::create_socket_path(Some(&daemon_project_path))?;
                            let pid_file = socket_path.with_extension("pid");
                            if pid_file.exists() {
                                // Daemon started successfully
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
                
                // Final check - make sure the daemon is still running
                match child.try_wait()? {
                    Some(exit_status) => {
                        // Process exited after starting, get the error
                        let output = child.wait_with_output()?;
                        let error_msg = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow::anyhow!("Daemon exited after starting: {}", error_msg.trim()));
                    }
                    None => {
                        // Daemon is still running, success
                        println!("Daemon started successfully");
                    }
                }
            } else {
                // Run daemon in foreground (default)
                println!("Starting daemon in foreground...");
                
                // Start daemon as child process in same process group (default foreground mode)
                let daemon_binary = if cfg!(debug_assertions) {
                    "./target/debug/claude-ntfy-daemon"
                } else {
                    "claude-ntfy-daemon"
                };
                let mut child = process::Command::new(daemon_binary)
                    .arg(daemon_project_path.to_string_lossy().to_string())
                    .spawn()?;
                    
                let child_id = child.id();
                println!("Daemon started with PID: {}", child_id);
                
                // Create shared state for signal handler
                let daemon_project_path_for_signal = daemon_project_path.clone();
                
                // Set up signal handling to gracefully stop the daemon
                ctrlc::set_handler(move || {
                    println!("\nReceived Ctrl+C, stopping daemon...");
                    
                    // Try to send shutdown signal to daemon via IPC first
                    match daemon::create_socket_path(Some(&daemon_project_path_for_signal)) {
                        Ok(socket_path) => {
                            let cmd_file = socket_path.with_extension("cmd");
                            let message = daemon::DaemonMessage::Shutdown;
                            
                            match serde_json::to_string(&message) {
                                Ok(content) => {
                                    match std::fs::write(&cmd_file, content) {
                                        Ok(_) => {
                                            println!("Sent shutdown signal via IPC, waiting for daemon to stop...");
                                            // Give daemon time to shutdown gracefully
                                            std::thread::sleep(std::time::Duration::from_millis(2000));
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to write shutdown command file: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to serialize shutdown message: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to get socket path: {}", e);
                        }
                    }
                    
                    // Always exit after attempting shutdown
                    println!("Exiting main process...");
                    std::process::exit(0);
                }).context("Failed to set Ctrl+C handler")?;
                
                // Wait for daemon to complete
                match child.wait() {
                    Ok(status) => {
                        if !status.success() {
                            return Err(anyhow::anyhow!("Daemon exited with error: {}", status));
                        }
                    }
                    Err(e) => {
                        eprintln!("Error waiting for daemon: {}", e);
                        return Err(e.into());
                    }
                }
            }
        }
        DaemonAction::Stop => {
            // Use current directory as default to match start command
            let daemon_project_path = project_path
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

            let socket_path = daemon::create_socket_path(Some(&daemon_project_path))?;
            let pid_file = socket_path.with_extension("pid");
            let cmd_file = socket_path.with_extension("cmd");

            // First check if daemon is running
            if !pid_file.exists() {
                println!("Daemon is not running");
                return Ok(());
            }

            let pid_str = std::fs::read_to_string(&pid_file)?;
            let pid = pid_str.trim();

            // Parse PID and check if process is actually running
            if let Ok(pid_num) = pid.parse::<u32>() {
                if daemon::is_process_running(pid_num) {
                    // Send shutdown signal
                    let message = daemon::DaemonMessage::Shutdown;
                    std::fs::write(&cmd_file, serde_json::to_string(&message)?)?;
                    println!("Daemon stop signal sent");

                    // Wait for daemon to stop (up to 5 seconds)
                    use std::time::{Duration, Instant};
                    let start_time = Instant::now();
                    let timeout = Duration::from_secs(5);

                    while start_time.elapsed() < timeout {
                        std::thread::sleep(Duration::from_millis(100));
                        if !daemon::is_process_running(pid_num) {
                            break;
                        }
                    }

                    // Verify process has stopped
                    if daemon::is_process_running(pid_num) {
                        println!("Warning: Daemon may still be running after stop signal");
                    } else {
                        println!("Daemon stopped successfully");
                        // Clean up PID file if process has stopped (only if it still exists)
                        if pid_file.exists() {
                            if let Err(e) = std::fs::remove_file(&pid_file) {
                                eprintln!("Warning: Failed to remove PID file: {}", e);
                            }
                        }
                    }
                } else {
                    // Process is not running, clean up PID file
                    if pid_file.exists() {
                        if let Err(e) = std::fs::remove_file(&pid_file) {
                            eprintln!("Warning: Failed to remove stale PID file: {}", e);
                        } else {
                            println!("Daemon was not running (cleaned up stale PID file)");
                        }
                    } else {
                        println!("Daemon was not running (no PID file found)");
                    }
                }
            } else {
                // Invalid PID format, clean up the file
                if pid_file.exists() {
                    if let Err(e) = std::fs::remove_file(&pid_file) {
                        eprintln!("Warning: Failed to remove invalid PID file: {}", e);
                    } else {
                        println!("Daemon was not running (invalid PID file removed)");
                    }
                } else {
                    println!("Daemon was not running (no PID file found)");
                }
            }
        }
        DaemonAction::Status => {
            // Use current directory as default to match start command
            let daemon_project_path = project_path
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

            let socket_path = daemon::create_socket_path(Some(&daemon_project_path))?;
            let pid_file = socket_path.with_extension("pid");
            if pid_file.exists() {
                let pid_str = std::fs::read_to_string(&pid_file)?;
                let pid = pid_str.trim();

                // Parse PID and check if process is actually running
                if let Ok(pid_num) = pid.parse::<u32>() {
                    if daemon::is_process_running(pid_num) {
                        println!("Daemon is running (PID: {})", pid);
                    } else {
                        // Process is not running, clean up stale PID file
                        if pid_file.exists() {
                            if let Err(e) = std::fs::remove_file(&pid_file) {
                                eprintln!("Warning: Failed to remove stale PID file: {}", e);
                            } else {
                                info!("Removed stale PID file for non-running process {}", pid);
                            }
                            println!("Daemon is not running (cleaned up stale PID file)");
                        } else {
                            println!("Daemon is not running (no PID file found)");
                        }
                    }
                } else {
                    // Invalid PID format, clean up the file
                    if pid_file.exists() {
                        if let Err(e) = std::fs::remove_file(&pid_file) {
                            eprintln!("Warning: Failed to remove invalid PID file: {}", e);
                        } else {
                            println!("Daemon is not running (invalid PID file removed)");
                        }
                    } else {
                        println!("Daemon is not running (no PID file found)");
                    }
                }
            } else {
                println!("Daemon is not running");
            }
        }
        DaemonAction::Reload => {
            // Use current directory as default to match start command
            let daemon_project_path = project_path
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

            let socket_path = daemon::create_socket_path(Some(&daemon_project_path))?;
            let cmd_file = socket_path.with_extension("cmd");
            let message = daemon::DaemonMessage::Reload;
            std::fs::write(&cmd_file, serde_json::to_string(&message)?)?;
            println!("Daemon reload signal sent");
        }
    }

    Ok(())
}

fn test_notification(
    project_path: Option<PathBuf>,
    message: String,
    title: Option<String>,
    priority: u8,
    topic: Option<String>,
) -> Result<()> {
    let config_manager = ConfigManager::new(project_path)?;
    let config = config_manager.config();

    let client = NtfyClient::new(
        config.ntfy.server_url.clone(),
        config.ntfy.auth_token.clone(),
        config.ntfy.timeout_secs,
        config.ntfy.send_format.clone(),
    )?;

    let topic = topic.unwrap_or_else(|| config.ntfy.default_topic.clone());
    let title = title.unwrap_or_else(|| "Claude Ntfy Test".to_string());

    client.send_simple(&topic, &title, &message, priority)?;

    println!("Test notification sent successfully");
    println!("Topic: {topic}");
    println!("Title: {title}");
    println!("Message: {message}");
    println!("Priority: {priority}");

    Ok(())
}

fn handle_templates(_project_path: Option<PathBuf>, show: Option<String>) -> Result<()> {
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

// Import daemon module types
mod daemon {
    use anyhow::{Context, Result};
    use serde::{Deserialize, Serialize};
    use std::path::PathBuf;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct NotificationTask {
        pub hook_name: String,
        pub hook_data: serde_json::Value,
        pub retry_count: u32,
        pub timestamp: chrono::DateTime<chrono::Local>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum DaemonMessage {
        Submit(NotificationTask),
        Ping,
        Shutdown,
        Reload,
        Status,
    }

    pub fn create_socket_path(project_path: Option<&PathBuf>) -> Result<PathBuf> {
        let base_path = if let Some(path) = project_path {
            path.join(".claude").join("ntfy-service")
        } else {
            let base_dirs =
                directories::BaseDirs::new().context("Failed to get base directories")?;
            base_dirs.home_dir().join(".claude").join("ntfy-service")
        };

        std::fs::create_dir_all(&base_path).context("Failed to create socket directory")?;

        Ok(base_path.join("daemon.sock"))
    }

    pub fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;

            // Use kill -0 to check if process exists
            Command::new("kill")
                .arg("-0")
                .arg(pid.to_string())
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }

        #[cfg(windows)]
        {
            use std::process::Command;

            // Use tasklist on Windows to check if process exists
            Command::new("tasklist")
                .arg("/FI")
                .arg(format!("PID eq {}", pid))
                .output()
                .map(|output| {
                    output.status.success()
                        && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
                })
                .unwrap_or(false)
        }
    }
}
