use anyhow::{Context, Result};
use flume::Receiver;
use std::path::PathBuf;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
use tokio::signal;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

// Import specific items from daemon modules
use super::templates::{MessageFormatter, TemplateEngine};
use super::clients::{traits::NotificationClient, AsyncNtfyClient};
use super::ipc::{IpcServer, create_socket_path};
use super::ntfy::NtfyMessage;
use super::shared::NotificationTask;

/// Auto-detect project path by looking for .claude/ntfy-service/config.toml
fn resolve_project_path(project_path: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(path) = project_path {
        return Some(path);
    }
    
    // Check if current directory has .claude/ntfy-service/config.toml
    if let Ok(current_dir) = std::env::current_dir() {
        let config_path = current_dir.join(".claude").join("ntfy-service").join("config.toml");
        if config_path.exists() {
            return Some(current_dir);
        }
    }
    
    // No project config found, use global config
    None
}

// NotificationTask is now imported from shared module

pub struct NotificationDaemon {
    template_engine: Arc<TemplateEngine>,
    message_formatter: Arc<MessageFormatter>,
    task_receiver: Receiver<NotificationTask>,
    shutdown_receiver: Receiver<()>,
    queue_size: Arc<AtomicUsize>,
    max_retries: u32,
    retry_delay: Duration,
}

impl NotificationDaemon {
    pub fn new(
        task_receiver: Receiver<NotificationTask>,
        shutdown_receiver: Receiver<()>,
        queue_size: Arc<AtomicUsize>,
    ) -> Result<Self> {
        let template_engine = Arc::new(TemplateEngine::new()?);
        let message_formatter = Arc::new(MessageFormatter::default());

        Ok(NotificationDaemon {
            template_engine,
            message_formatter,
            task_receiver,
            shutdown_receiver,
            queue_size,
            max_retries: 3, // Default retry attempts
            retry_delay: Duration::from_secs(5), // Default retry delay
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
        match self.task_receiver.recv_async().await.ok() {
            Some(task) => {
                // Decrement queue size when task is dequeued
                self.queue_size.fetch_sub(1, Ordering::Relaxed);
                Some(task)
            }
            None => None,
        }
    }

    async fn process_task(&self, task: NotificationTask) {
        debug!("Processing notification task: {} from project: {:?}", 
               task.hook_name, task.project_path);

        // Deserialize hook data from JSON string
        let hook_data: serde_json::Value = match serde_json::from_str(&task.hook_data) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to deserialize hook data: {}", e);
                return;
            }
        };

        // Create dynamic ntfy client based on task configuration
        let ntfy_client = match self.create_ntfy_client(&task.ntfy_config).await {
            Ok(client) => client,
            Err(e) => {
                error!("Failed to create ntfy client for task {}: {}", task.hook_name, e);
                return;
            }
        };

        // Prepare notification message
        let message = match self.prepare_message(&task, &hook_data).await {
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
            match ntfy_client.send(&message).await {
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

    /// Create ntfy client dynamically based on task configuration
    async fn create_ntfy_client(&self, config: &super::shared::NtfyTaskConfig) -> Result<AsyncNtfyClient> {
        use super::clients::ntfy::NtfyClientConfig;
        use super::clients::traits::RetryConfig;
        
        let client_config = NtfyClientConfig {
            server_url: config.server_url.clone(),
            auth_token: config.auth_token.clone(),
            timeout_secs: Some(30), // Default timeout
            send_format: config.send_format.clone(),
            retry_config: RetryConfig::exponential(3, 1000), // 3 retries, 1s base delay
            user_agent: Some("claude-ntfy-service".to_string()),
        };

        let client = AsyncNtfyClient::new(client_config)
            .context("Failed to create ntfy client")?;
        
        Ok(client)
    }

    async fn prepare_message(&self, task: &NotificationTask, hook_data: &serde_json::Value) -> Result<NtfyMessage> {

        // Get template name and render message body  
        let template_name = task.hook_name.replace('_', "-");
        let formatted_data = self
            .template_engine
            .format_hook_data(&task.hook_name, &hook_data);

        // Use default template rendering (no custom templates in global daemon)
        let body = self.template_engine
            .render(&template_name, &formatted_data, None)
            .unwrap_or_else(|_| {
                // Fallback to simple message if template fails
                format!("Hook: {}\nData: {}", task.hook_name, hook_data)
            });

        // Format title
        let title = self
            .message_formatter
            .format_title(&task.hook_name, &formatted_data);

        // Get configuration from task (no longer from config_manager)
        let topic = &task.ntfy_config.topic;
        let priority = task.ntfy_config.priority.unwrap_or(3);
        let tags = task.ntfy_config.tags.clone();

        Ok(NtfyMessage {
            topic: topic.clone(),
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
            send_format: task.ntfy_config.send_format.clone(),
        })
    }

    async fn drain_queue(&self) {
        info!("Draining remaining notification queue");

        while let Ok(task) = self.task_receiver.try_recv() {
            // Decrement queue size when task is dequeued during drain
            self.queue_size.fetch_sub(1, Ordering::Relaxed);
            self.process_task(task).await;
        }
    }
}

// DaemonMessage and DaemonResponse are now imported from shared module

// Main entry point for the daemon
pub async fn main() -> Result<()> {
    // Parse command line arguments for global daemon
    let args: Vec<String> = std::env::args().collect();
    let mut background_mode = false;
    let mut _global_mode = false;
    
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--background" | "-b" => {
                background_mode = true;
            }
            "--global" | "-g" => {
                _global_mode = true;
            }
            _ => {
                error!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    
    // Check for existing global daemon before starting
    check_existing_daemon(None)?; // None = global daemon

    // Initialize simple tracing (no config dependency)
    let _file_guard = if background_mode {
        // Background mode: log to file in global daemon directory
        let base_dirs = directories::BaseDirs::new().context("Failed to get base directories")?;
        let log_dir = base_dirs.home_dir().join(".claude").join("ntfy-service");
        std::fs::create_dir_all(&log_dir).context("Failed to create log directory")?;
        let log_path = log_dir.join("daemon.log");
        
        
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
                    .add_directive(tracing::Level::INFO.into()),
            )
            .init();
            
        info!("Starting global Claude Ntfy daemon in background mode with file logging to: {:?}", log_path);
        Some(guard)
    } else {
        // Foreground mode: console logging only
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::INFO.into()),
            )
            .init();
            
        info!("Starting Claude Ntfy daemon with console logging only");
        None
    };


    // Create communication channels
    let (task_sender, task_receiver) = flume::unbounded::<NotificationTask>();
    let (shutdown_sender, shutdown_receiver) = flume::bounded::<()>(1);

    // Create shared queue size counter
    let queue_size = Arc::new(AtomicUsize::new(0));

    // Store sender for IPC server
    let task_sender_clone = task_sender.clone();
    let shutdown_sender_clone = shutdown_sender.clone();
    let queue_size_clone = queue_size.clone();
    let socket_path = create_socket_path(None)?; // Global socket path
    let socket_path_for_ipc = socket_path.clone();

    // Create IPC server shutdown channel
    let (ipc_shutdown_tx, ipc_shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Start high-performance IPC server in background
    let ipc_handle = tokio::spawn(async move {
        match IpcServer::new(socket_path_for_ipc, task_sender_clone, shutdown_sender_clone, queue_size_clone).await {
            Ok(mut server) => {
                // Add IPC shutdown receiver to server
                server.set_shutdown_receiver(ipc_shutdown_rx);
                if let Err(e) = server.run().await {
                    error!("IPC server error: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to create IPC server: {}", e);
            }
        }
    });

    // Create PID file for daemon status tracking
    let pid_file = socket_path.with_extension("pid");
    let current_pid = std::process::id();
    std::fs::write(&pid_file, current_pid.to_string())
        .context("Failed to create PID file")?;
    info!("Daemon started with PID: {} (PID file: {:?})", current_pid, pid_file);

    // Create and run daemon
    let daemon = NotificationDaemon::new(task_receiver, shutdown_receiver, queue_size)?;
    let daemon_result = daemon.run().await;

    // Send shutdown signal to IPC server
    if let Err(e) = ipc_shutdown_tx.send(()).await {
        warn!("Failed to send shutdown signal to IPC server: {}", e);
    } else {
        info!("Sent shutdown signal to IPC server");
    }

    // Wait for IPC server to finish
    let _ = ipc_handle.await;

    // Clean up PID file on shutdown
    if pid_file.exists() {
        if let Err(e) = std::fs::remove_file(&pid_file) {
            warn!("Failed to remove PID file during shutdown: {}", e);
        } else {
            info!("Removed PID file during shutdown");
        }
    }

    daemon_result
}

// Legacy file-based IPC server has been replaced with high-performance Unix socket IPC
// This function is no longer used but kept for compatibility during transition

// create_socket_path is now provided by the ipc module

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
