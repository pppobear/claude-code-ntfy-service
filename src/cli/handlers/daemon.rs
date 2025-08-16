//! Daemon management handler
//!
//! This module handles all daemon-related commands including start, stop,
//! status checks, and reload operations.

use super::super::{CliContext, DaemonAction};
use crate::daemon::{
    DaemonResponse, NotificationTask,
    create_socket_path, is_process_running
};
use crate::shared::ipc::convenience::{get_daemon_status, shutdown_daemon, reload_daemon};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process;
use tracing::{debug, error, info, warn};

/// Handler for daemon operations
pub struct DaemonHandler<'a> {
    context: &'a CliContext,
}

impl<'a> DaemonHandler<'a> {
    /// Create new daemon handler
    pub fn new(context: &'a CliContext) -> Self {
        Self { context }
    }

    /// Handle daemon management operations
    pub async fn handle_daemon(&self, action: DaemonAction) -> Result<()> {
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
    pub async fn handle_daemon_start(&self, detach: bool) -> Result<()> {
        if detach {
            self.start_daemon_detached()
        } else {
            self.start_daemon_foreground().await
        }
    }

    /// Handle daemon stop command
    pub async fn handle_daemon_stop(&self) -> Result<()> {
        let (pid_file, _socket_path) = self.get_daemon_paths()?;

        match self.check_daemon_process(&pid_file)? {
            Some(pid_num) => {
                // Try to send shutdown signal via Unix socket IPC first
                let (_, socket_path) = self.get_daemon_paths()?;
                match shutdown_daemon(&socket_path).await {
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
    pub async fn handle_daemon_status(&self) -> Result<()> {
        let (pid_file, _) = self.get_daemon_paths()?;
        
        match self.check_daemon_process(&pid_file)? {
            Some(pid_num) => {
                // Try to get detailed status via IPC
                let (_, socket_path) = self.get_daemon_paths()?;
                match get_daemon_status(&socket_path).await {
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
    pub async fn handle_daemon_reload(&self) -> Result<()> {
        let (pid_file, _) = self.get_daemon_paths()?;
        
        match self.check_daemon_process(&pid_file)? {
            Some(_pid_num) => {
                // Send reload signal via IPC
                let (_, socket_path) = self.get_daemon_paths()?;
                match reload_daemon(&socket_path).await {
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

    /// Get daemon file paths (pid_file, socket_path)
    fn get_daemon_paths(&self) -> Result<(PathBuf, PathBuf)> {
        // Use project-specific socket path if available, otherwise global
        let socket_path = create_socket_path(self.context.project_path.as_ref())?;
        let pid_file = socket_path.with_extension("pid");
        Ok((pid_file, socket_path))
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

// Implement the handler factory trait to reduce boilerplate
super::traits::impl_context_handler!(DaemonHandler<'a>);
