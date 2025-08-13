use anyhow::{Context, Result};
use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

mod config;
mod ntfy;
mod templates;

use config::ConfigManager;
use ntfy::{AsyncNtfyClient, NtfyMessage};
use templates::{MessageFormatter, TemplateEngine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTask {
    pub hook_name: String,
    pub hook_data: serde_json::Value,
    pub retry_count: u32,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

pub struct NotificationDaemon {
    config_manager: Arc<ConfigManager>,
    ntfy_client: Arc<AsyncNtfyClient>,
    template_engine: Arc<TemplateEngine>,
    message_formatter: Arc<MessageFormatter>,
    task_receiver: Receiver<NotificationTask>,
    shutdown_receiver: Receiver<()>,
    max_retries: u32,
    retry_delay: Duration,
}

impl NotificationDaemon {
    pub fn new(
        project_path: Option<PathBuf>,
        task_receiver: Receiver<NotificationTask>,
        shutdown_receiver: Receiver<()>,
    ) -> Result<Self> {
        let config_manager = Arc::new(ConfigManager::new(project_path)?);
        let config = config_manager.config().clone();

        let ntfy_client = Arc::new(AsyncNtfyClient::new(
            config.ntfy.server_url.clone(),
            config.ntfy.auth_token.clone(),
            config.ntfy.timeout_secs,
            config.ntfy.send_format.clone(),
        )?);

        let template_engine = Arc::new(TemplateEngine::new()?);
        let message_formatter = Arc::new(MessageFormatter::default());

        Ok(NotificationDaemon {
            config_manager,
            ntfy_client,
            template_engine,
            message_formatter,
            task_receiver,
            shutdown_receiver,
            max_retries: config.daemon.retry_attempts,
            retry_delay: Duration::from_secs(config.daemon.retry_delay_secs),
        })
    }

    pub async fn run(self) -> Result<()> {
        info!("Notification daemon started");

        // Set up graceful shutdown
        let ctrl_c = signal::ctrl_c();
        tokio::pin!(ctrl_c);

        loop {
            tokio::select! {
                // Handle incoming notification tasks
                task = self.receive_task() => {
                    if let Some(task) = task {
                        self.process_task(task).await;
                    }
                }

                // Handle IPC shutdown signal
                _ = self.shutdown_receiver.recv_async() => {
                    info!("Received IPC shutdown signal, stopping daemon");
                    break;
                }

                // Handle Ctrl+C shutdown signal
                _ = &mut ctrl_c => {
                    info!("Received Ctrl+C signal, stopping daemon");
                    break;
                }
            }
        }

        // Process remaining tasks before shutdown
        self.drain_queue().await;

        info!("Notification daemon stopped");
        Ok(())
    }

    async fn receive_task(&self) -> Option<NotificationTask> {
        self.task_receiver.recv_async().await.ok()
    }

    async fn process_task(&self, task: NotificationTask) {
        debug!("Processing notification task: {}", task.hook_name);

        // Check if hook should be processed
        if !self
            .config_manager
            .should_process_hook(&task.hook_name, &task.hook_data)
        {
            debug!("Hook {} filtered out, skipping", task.hook_name);
            return;
        }

        // Prepare notification message
        let message = match self.prepare_message(&task).await {
            Ok(msg) => msg,
            Err(e) => {
                error!(
                    "Failed to prepare message for hook {}: {}",
                    task.hook_name, e
                );
                return;
            }
        };

        // Send notification with retry logic
        let mut attempt = 0;
        loop {
            match self.ntfy_client.send(&message).await {
                Ok(_) => {
                    info!(
                        "Successfully sent notification for hook: {}",
                        task.hook_name
                    );
                    break;
                }
                Err(e) => {
                    attempt += 1;
                    if attempt > self.max_retries {
                        error!(
                            "Failed to send notification for hook {} after {} attempts: {}",
                            task.hook_name, self.max_retries, e
                        );
                        break;
                    }

                    warn!(
                        "Failed to send notification for hook {} (attempt {}/{}): {}",
                        task.hook_name, attempt, self.max_retries, e
                    );

                    sleep(self.retry_delay).await;
                }
            }
        }
    }

    async fn prepare_message(&self, task: &NotificationTask) -> Result<NtfyMessage> {
        let config = self.config_manager.config();

        // Get template name and render message body
        let template_name = task.hook_name.replace('_', "-");
        let formatted_data = self
            .template_engine
            .format_hook_data(&task.hook_name, &task.hook_data);

        let body = if config.templates.use_custom {
            if let Some(custom_template) = config.templates.custom_templates.get(&task.hook_name) {
                let mut hb = handlebars::Handlebars::new();
                hb.set_strict_mode(false);
                hb.render_template(custom_template, &formatted_data)
                    .unwrap_or_else(|e| {
                        error!("Failed to render custom template: {}", e);
                        self.template_engine
                            .render(
                                &template_name,
                                &formatted_data,
                                Some(&config.templates.variables),
                            )
                            .unwrap_or_else(|_| format!("Hook: {}", task.hook_name))
                    })
            } else {
                self.template_engine
                    .render(
                        &template_name,
                        &formatted_data,
                        Some(&config.templates.variables),
                    )
                    .context("Failed to render template")?
            }
        } else {
            self.template_engine
                .render(
                    &template_name,
                    &formatted_data,
                    Some(&config.templates.variables),
                )
                .context("Failed to render template")?
        };

        // Format title
        let title = self
            .message_formatter
            .format_title(&task.hook_name, &formatted_data);

        // Get topic, priority, and tags
        let topic = self.config_manager.get_hook_topic(&task.hook_name);
        let priority = self.config_manager.get_hook_priority(&task.hook_name);
        let tags = self
            .message_formatter
            .get_tags(&task.hook_name)
            .or_else(|| config.ntfy.default_tags.clone());

        Ok(NtfyMessage {
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
        })
    }

    async fn drain_queue(&self) {
        info!("Draining remaining notification queue");

        while let Ok(task) = self.task_receiver.try_recv() {
            self.process_task(task).await;
        }
    }
}

// IPC message for communication between CLI and daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonMessage {
    Submit(NotificationTask),
    Ping,
    Shutdown,
    Reload,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    Ok,
    Error(String),
    Status {
        queue_size: usize,
        is_running: bool,
        uptime_secs: u64,
    },
}

// Main entry point for the daemon
#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments to get project path and background mode
    let args: Vec<String> = std::env::args().collect();
    let mut project_path = None;
    let mut background_mode = false;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--background" | "-b" => {
                background_mode = true;
            }
            arg if !arg.starts_with("-") => {
                project_path = Some(PathBuf::from(arg));
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    
    // Check for existing daemon before starting
    check_existing_daemon(project_path.as_ref())?;

    // Load configuration to check for log path
    let config_manager = Arc::new(ConfigManager::new(project_path.clone())?);
    let config = config_manager.config().clone();

    // Initialize tracing with appropriate logging based on mode and configuration
    let _file_guard = if let Some(log_path_str) = &config.daemon.log_path {
        let log_path = PathBuf::from(log_path_str);
        
        // Ensure the log directory exists
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create log directory")?;
        }
        
        if background_mode {
            // Background mode: log only to file
            let file_appender = tracing_appender::rolling::daily(
                log_path.parent().unwrap_or_else(|| std::path::Path::new(".")),
                log_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("daemon.log"))
            );
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            
            tracing_subscriber::fmt()
                .with_writer(non_blocking)
                .with_ansi(false) // Disable colors in file output
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(config.daemon.log_level.parse().unwrap_or(tracing::Level::INFO.into())),
                )
                .init();
                
            info!("Starting Claude Ntfy daemon in background mode with file logging to: {:?}", log_path);
            Some(guard)
        } else {
            // Foreground mode (default): log to both console and file
            use tracing_subscriber::prelude::*;
            
            // Set up file logging
            let file_appender = tracing_appender::rolling::daily(
                log_path.parent().unwrap_or_else(|| std::path::Path::new(".")),
                log_path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("daemon.log"))
            );
            let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
            
            // Set up console logging
            let console_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stdout);
                
            // Set up file logging layer
            let file_layer = tracing_subscriber::fmt::layer()
                .with_writer(file_writer)
                .with_ansi(false); // Disable colors in file output
            
            // Combine both layers
            let env_filter = tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(config.daemon.log_level.parse().unwrap_or(tracing::Level::INFO.into()));
            
            tracing_subscriber::registry()
                .with(env_filter)
                .with(console_layer)
                .with(file_layer)
                .init();
                
            info!("Starting Claude Ntfy daemon in foreground mode with dual logging (console + file: {:?})", log_path);
            Some(guard)
        }
    } else {
        // No file logging configured, use console only (should only happen in foreground mode)
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(config.daemon.log_level.parse().unwrap_or(tracing::Level::INFO.into())),
            )
            .init();
            
        info!("Starting Claude Ntfy daemon with console logging only");
        None
    };


    // Create communication channels
    let (task_sender, task_receiver) = flume::unbounded::<NotificationTask>();
    let (shutdown_sender, shutdown_receiver) = flume::bounded::<()>(1);

    // Store sender for IPC server
    let task_sender_clone = task_sender.clone();
    let shutdown_sender_clone = shutdown_sender.clone();
    let socket_path = create_socket_path(project_path.as_ref())?;

    // Start IPC server in background
    let ipc_handle = tokio::spawn(async move {
        if let Err(e) = run_ipc_server(socket_path, task_sender_clone, shutdown_sender_clone).await
        {
            error!("IPC server error: {}", e);
        }
    });

    // Create and run daemon
    let daemon = NotificationDaemon::new(project_path.clone(), task_receiver, shutdown_receiver)?;
    let daemon_result = daemon.run().await;

    // Wait for IPC server to finish
    let _ = ipc_handle.await;

    daemon_result
}

// Simplified IPC server (in production, use proper Unix socket or named pipe)
async fn run_ipc_server(
    socket_path: PathBuf,
    task_sender: Sender<NotificationTask>,
    shutdown_sender: Sender<()>,
) -> Result<()> {
    // For now, we'll use a simple file-based approach
    // In production, implement proper Unix socket or named pipe communication

    info!("IPC server listening on {:?}", socket_path);

    // Clean up any leftover command files from previous sessions
    let cmd_file = socket_path.with_extension("cmd");
    if cmd_file.exists() {
        if let Err(e) = std::fs::remove_file(&cmd_file) {
            warn!("Failed to clean up leftover command file: {}", e);
        } else {
            info!("Cleaned up leftover command file from previous session");
        }
    }

    // Create a marker file to indicate daemon is running
    let marker_file = socket_path.with_extension("pid");
    std::fs::write(&marker_file, std::process::id().to_string())
        .context("Failed to write PID file")?;

    // Clean up on exit
    let cmd_file_clone = cmd_file.clone();
    let _guard = scopeguard::guard((marker_file, cmd_file_clone), |(pid_file, cmd_file)| {
        let _ = std::fs::remove_file(pid_file);
        let _ = std::fs::remove_file(cmd_file);
    });

    // Keep the server running
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check for command file (simplified IPC)
        let cmd_file = socket_path.with_extension("cmd");
        if cmd_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&cmd_file) {
                if let Ok(msg) = serde_json::from_str::<DaemonMessage>(&content) {
                    match msg {
                        DaemonMessage::Submit(task) => {
                            let _ = task_sender.send_async(task).await;
                        }
                        DaemonMessage::Shutdown => {
                            info!("Received shutdown command via IPC");
                            let _ = shutdown_sender.send_async(()).await;
                            break;
                        }
                        DaemonMessage::Reload => {
                            info!("Received reload command (not implemented yet)");
                        }
                        _ => {
                            debug!("Received other IPC message: {:?}", msg);
                        }
                    }
                }
                let _ = std::fs::remove_file(&cmd_file);
            }
        }
    }

    Ok(())
}

pub fn create_socket_path(project_path: Option<&PathBuf>) -> Result<PathBuf> {
    let base_path = if let Some(path) = project_path {
        path.join(".claude").join("ntfy-service")
    } else {
        let base_dirs = directories::BaseDirs::new().context("Failed to get base directories")?;
        base_dirs.home_dir().join(".claude").join("ntfy-service")
    };

    std::fs::create_dir_all(&base_path).context("Failed to create socket directory")?;

    Ok(base_path.join("daemon.sock"))
}

fn is_process_running(pid: u32) -> bool {
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

fn check_existing_daemon(project_path: Option<&PathBuf>) -> Result<()> {
    let socket_path = create_socket_path(project_path)?;
    let pid_file = socket_path.with_extension("pid");
    
    if !pid_file.exists() {
        // No PID file exists, so no daemon is running
        return Ok(());
    }
    
    match std::fs::read_to_string(&pid_file) {
        Ok(pid_str) => {
            let pid = pid_str.trim();
            if let Ok(pid_num) = pid.parse::<u32>() {
                if is_process_running(pid_num) {
                    return Err(anyhow::anyhow!(
                        "Another claude-ntfy daemon is already running (PID: {}). \
                        Stop it first with 'claude-ntfy daemon stop'", 
                        pid_num
                    ));
                } else {
                    // Process not running, clean up stale PID file
                    if let Err(e) = std::fs::remove_file(&pid_file) {
                        warn!("Failed to remove stale PID file: {}", e);
                    } else {
                        info!("Removed stale PID file for non-running process {}", pid_num);
                    }
                }
            } else {
                // Invalid PID format, clean up the file
                if let Err(e) = std::fs::remove_file(&pid_file) {
                    warn!("Failed to remove invalid PID file: {}", e);
                } else {
                    info!("Removed invalid PID file");
                }
            }
        }
        Err(e) => {
            warn!("Failed to read PID file: {}", e);
            // Try to remove the unreadable file
            if let Err(e) = std::fs::remove_file(&pid_file) {
                warn!("Failed to remove unreadable PID file: {}", e);
            }
        }
    }
    
    Ok(())
}
