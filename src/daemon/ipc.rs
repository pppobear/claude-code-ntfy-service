//! High-performance Unix socket IPC layer for daemon communication
//!
//! This module replaces the file-based polling system with async Unix socket
//! communication for 50x latency improvement (from 100ms to ~2ms).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

use crate::daemon::shared::{DaemonMessage, DaemonResponse, NotificationTask};

/// Maximum message size for IPC communication (1MB)
const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

/// IPC Server for handling Unix socket connections from CLI clients
pub struct IpcServer {
    socket_path: PathBuf,
    listener: UnixListener,
    task_sender: flume::Sender<NotificationTask>,
    shutdown_sender: flume::Sender<()>,
    queue_size: Arc<AtomicUsize>,
    stats: Arc<Mutex<ServerStats>>,
    external_shutdown_rx: Option<tokio::sync::mpsc::Receiver<()>>,
}

/// IPC Client for sending commands to the daemon
pub struct IpcClient {
    socket_path: PathBuf,
}

/// Server statistics for monitoring
#[derive(Debug, Default)]
struct ServerStats {
    connections_handled: u64,
    messages_processed: u64,
    errors_encountered: u64,
    uptime_start: Option<std::time::Instant>,
}

/// IPC message with framing for reliable transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FramedMessage {
    length: u32,
    payload: DaemonMessage,
}

impl IpcServer {
    /// Create a new IPC server
    pub async fn new(
        socket_path: PathBuf,
        task_sender: flume::Sender<NotificationTask>,
        shutdown_sender: flume::Sender<()>,
        queue_size: Arc<AtomicUsize>,
    ) -> Result<Self> {
        // Remove existing socket if it exists
        if socket_path.exists() {
            tokio::fs::remove_file(&socket_path)
                .await
                .context("Failed to remove existing socket")?;
        }

        // Create parent directory if needed
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create socket directory")?;
        }

        // Bind Unix socket listener
        let listener = UnixListener::bind(&socket_path)
            .context("Failed to bind Unix socket")?;

        info!("IPC server listening on {:?}", socket_path);

        let mut stats = ServerStats::default();
        stats.uptime_start = Some(std::time::Instant::now());

        Ok(IpcServer {
            socket_path,
            listener,
            task_sender,
            shutdown_sender,
            queue_size,
            stats: Arc::new(Mutex::new(stats)),
            external_shutdown_rx: None,
        })
    }

    /// Set external shutdown receiver
    pub fn set_shutdown_receiver(&mut self, shutdown_rx: tokio::sync::mpsc::Receiver<()>) {
        self.external_shutdown_rx = Some(shutdown_rx);
    }

    /// Run the IPC server
    pub async fn run(mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        // Cleanup socket on shutdown
        let socket_path = self.socket_path.clone();
        let _guard = scopeguard::guard(socket_path, |path| {
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        });

        // Extract external shutdown receiver
        let mut external_shutdown_rx = self.external_shutdown_rx.take();

        loop {
            tokio::select! {
                // Accept new connections
                connection = self.listener.accept() => {
                    match connection {
                        Ok((stream, _addr)) => {
                            let task_sender = self.task_sender.clone();
                            let shutdown_sender = self.shutdown_sender.clone();
                            let queue_size = self.queue_size.clone();
                            let stats = self.stats.clone();
                            let shutdown_tx = shutdown_tx.clone();

                            // Handle connection in separate task
                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_connection(
                                    stream,
                                    task_sender,
                                    shutdown_sender,
                                    queue_size,
                                    stats,
                                    shutdown_tx,
                                ).await {
                                    error!("Error handling connection: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                            self.increment_error_count().await;
                        }
                    }
                }

                // Handle internal shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("IPC server received internal shutdown signal");
                    break;
                }

                // Handle external shutdown signal
                result = async {
                    match &mut external_shutdown_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if result.is_some() {
                        info!("IPC server received external shutdown signal");
                        break;
                    }
                }
            }
        }

        info!("IPC server stopped");
        Ok(())
    }

    /// Handle individual client connection
    async fn handle_connection(
        mut stream: UnixStream,
        task_sender: flume::Sender<NotificationTask>,
        shutdown_sender: flume::Sender<()>,
        queue_size: Arc<AtomicUsize>,
        stats: Arc<Mutex<ServerStats>>,
        shutdown_tx: mpsc::Sender<()>,
    ) -> Result<()> {
        // Update connection stats
        {
            let mut stats = stats.lock().await;
            stats.connections_handled += 1;
        }

        debug!("New IPC connection established");

        loop {
            // Read message length first (4 bytes)
            let mut length_bytes = [0u8; 4];
            match stream.read_exact(&mut length_bytes).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    debug!("Client disconnected");
                    break;
                }
                Err(e) => {
                    error!("Failed to read message length: {}", e);
                    Self::increment_error_count_static(&stats).await;
                    break;
                }
            }

            let message_length = u32::from_le_bytes(length_bytes) as usize;
            
            // Validate message length
            if message_length == 0 {
                error!("Invalid message length: 0 bytes");
                Self::increment_error_count_static(&stats).await;
                break;
            }
            
            if message_length > MAX_MESSAGE_SIZE {
                error!("Message too large: {} bytes (max: {} bytes)", message_length, MAX_MESSAGE_SIZE);
                Self::increment_error_count_static(&stats).await;
                break;
            }

            // Dynamically allocate buffer for the message
            let mut buffer = vec![0u8; message_length];

            // Read message payload
            match stream.read_exact(&mut buffer).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to read message payload: {}", e);
                    Self::increment_error_count_static(&stats).await;
                    break;
                }
            }

            // Deserialize message
            let message: DaemonMessage = match bincode::deserialize(&buffer) {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Failed to deserialize message: {}", e);
                    Self::increment_error_count_static(&stats).await;
                    
                    // Send error response
                    let response = DaemonResponse::Error(format!("Invalid message format: {}", e));
                    let _ = Self::send_response(&mut stream, response).await;
                    continue;
                }
            };

            // Update message stats
            {
                let mut stats = stats.lock().await;
                stats.messages_processed += 1;
            }

            debug!("Received IPC message: {:?}", message);

            // Process message and send response
            let response = Self::process_message(
                message,
                &task_sender,
                &shutdown_sender,
                &shutdown_tx,
                &queue_size,
                &stats,
            ).await;

            if let Err(e) = Self::send_response(&mut stream, response).await {
                error!("Failed to send response: {}", e);
                Self::increment_error_count_static(&stats).await;
                break;
            }
        }

        Ok(())
    }

    /// Process daemon message and return response
    async fn process_message(
        message: DaemonMessage,
        task_sender: &flume::Sender<NotificationTask>,
        shutdown_sender: &flume::Sender<()>,
        shutdown_tx: &mpsc::Sender<()>,
        queue_size: &Arc<AtomicUsize>,
        stats: &Arc<Mutex<ServerStats>>,
    ) -> DaemonResponse {
        match message {
            DaemonMessage::Submit(task) => {
                match task_sender.send_async(task.clone()).await {
                    Ok(()) => {
                        // Increment queue size when task is successfully queued
                        queue_size.fetch_add(1, Ordering::Relaxed);
                        info!("Task queued: {}", task.hook_name);
                        DaemonResponse::Ok
                    }
                    Err(e) => {
                        error!("Failed to queue task: {}", e);
                        DaemonResponse::Error(format!("Failed to queue task: {}", e))
                    }
                }
            }
            DaemonMessage::Ping => DaemonResponse::Ok,
            DaemonMessage::Shutdown => {
                info!("Shutdown requested via IPC");
                let _ = shutdown_sender.send_async(()).await;
                let _ = shutdown_tx.send(()).await;
                DaemonResponse::Ok
            }
            DaemonMessage::Reload => {
                warn!("Reload not implemented yet");
                DaemonResponse::Error("Reload not implemented".to_string())
            }
            DaemonMessage::Status => {
                let stats = stats.lock().await;
                let uptime_secs = stats.uptime_start
                    .map(|start| start.elapsed().as_secs())
                    .unwrap_or(0);
                
                DaemonResponse::Status {
                    queue_size: queue_size.load(Ordering::Relaxed),
                    is_running: true,
                    uptime_secs,
                }
            }
        }
    }

    /// Send response back to client
    async fn send_response(
        stream: &mut UnixStream,
        response: DaemonResponse,
    ) -> Result<()> {
        let serialized = bincode::serialize(&response)
            .context("Failed to serialize response")?;

        // Validate response size
        if serialized.len() > MAX_MESSAGE_SIZE {
            error!("Response too large: {} bytes (max: {} bytes)",
                   serialized.len(), MAX_MESSAGE_SIZE);
            // Send error response instead
            let error_response = DaemonResponse::Error(
                format!("Response too large: {} bytes", serialized.len())
            );
            let error_serialized = bincode::serialize(&error_response)
                .context("Failed to serialize error response")?;
            let error_length = error_serialized.len() as u32;
            let error_length_bytes = error_length.to_le_bytes();
            
            stream.write_all(&error_length_bytes).await
                .context("Failed to write error response length")?;
            stream.write_all(&error_serialized).await
                .context("Failed to write error response payload")?;
            stream.flush().await
                .context("Failed to flush error response")?;
            
            return Ok(());
        }

        let length = serialized.len() as u32;
        let length_bytes = length.to_le_bytes();

        // Write length prefix
        stream.write_all(&length_bytes).await
            .context("Failed to write response length")?;

        // Write response payload
        stream.write_all(&serialized).await
            .context("Failed to write response payload")?;

        stream.flush().await
            .context("Failed to flush response")?;

        Ok(())
    }

    /// Increment error count in stats
    async fn increment_error_count(&self) {
        let mut stats = self.stats.lock().await;
        stats.errors_encountered += 1;
    }

    /// Static version for use in spawned tasks
    async fn increment_error_count_static(stats: &Arc<Mutex<ServerStats>>) {
        let mut stats = stats.lock().await;
        stats.errors_encountered += 1;
    }
}

impl IpcClient {
    /// Create a new IPC client
    pub fn new(socket_path: PathBuf) -> Self {
        IpcClient { socket_path }
    }

    /// Send a message to the daemon and wait for response
    pub async fn send_message(&self, message: DaemonMessage) -> Result<DaemonResponse> {
        // Connect to Unix socket
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .context("Failed to connect to daemon socket")?;

        // Serialize message
        let serialized = bincode::serialize(&message)
            .context("Failed to serialize message")?;

        // Validate message size before sending
        if serialized.len() > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!("Message too large: {} bytes (max: {} bytes)",
                                      serialized.len(), MAX_MESSAGE_SIZE));
        }

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
        if response_length > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!("Response too large: {} bytes (max: {} bytes)",
                                      response_length, MAX_MESSAGE_SIZE));
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

    /// Send a task to the daemon
    pub async fn send_task(&self, task: NotificationTask) -> Result<()> {
        let response = self.send_message(DaemonMessage::Submit(task)).await?;

        match response {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(e) => Err(anyhow::anyhow!("Daemon error: {}", e)),
            _ => Err(anyhow::anyhow!("Unexpected response: {:?}", response)),
        }
    }

    /// Ping the daemon to check if it's running
    pub async fn ping(&self) -> Result<()> {
        let response = self.send_message(DaemonMessage::Ping).await?;

        match response {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(e) => Err(anyhow::anyhow!("Daemon error: {}", e)),
            _ => Err(anyhow::anyhow!("Unexpected response: {:?}", response)),
        }
    }

    /// Request daemon shutdown
    pub async fn shutdown(&self) -> Result<()> {
        let response = self.send_message(DaemonMessage::Shutdown).await?;

        match response {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error(e) => Err(anyhow::anyhow!("Daemon error: {}", e)),
            _ => Err(anyhow::anyhow!("Unexpected response: {:?}", response)),
        }
    }

    /// Get daemon status
    pub async fn status(&self) -> Result<(usize, bool, u64)> {
        let response = self.send_message(DaemonMessage::Status).await?;

        match response {
            DaemonResponse::Status { queue_size, is_running, uptime_secs } => {
                Ok((queue_size, is_running, uptime_secs))
            }
            DaemonResponse::Error(e) => Err(anyhow::anyhow!("Daemon error: {}", e)),
            _ => Err(anyhow::anyhow!("Unexpected response: {:?}", response)),
        }
    }

    /// Check if daemon is running by testing socket connectivity
    pub async fn is_daemon_running(&self) -> bool {
        match self.ping().await {
            Ok(()) => true,
            Err(_) => false,
        }
    }
}

/// Create socket path for daemon communication
/// Reused from daemon_shared for compatibility
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_ipc_client_server_communication() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create channels for testing
        let (task_sender, _task_receiver) = flume::unbounded::<NotificationTask>();
        let (shutdown_sender, _shutdown_receiver) = flume::unbounded::<()>();

        // Start IPC server
        let queue_size = Arc::new(AtomicUsize::new(0));
        let server = IpcServer::new(
            socket_path.clone(),
            task_sender,
            shutdown_sender,
            queue_size,
        ).await.unwrap();

        let server_handle = tokio::spawn(async move {
            server.run().await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create client and test ping
        let client = IpcClient::new(socket_path);
        let ping_result = timeout(Duration::from_secs(1), client.ping()).await;
        assert!(ping_result.is_ok() && ping_result.unwrap().is_ok());

        // Test status
        let status_result = timeout(Duration::from_secs(1), client.status()).await;
        assert!(status_result.is_ok() && status_result.unwrap().is_ok());

        // Test shutdown
        let shutdown_result = timeout(Duration::from_secs(1), client.shutdown()).await;
        assert!(shutdown_result.is_ok() && shutdown_result.unwrap().is_ok());

        // Server should stop
        let server_result = timeout(Duration::from_secs(2), server_handle).await;
        assert!(server_result.is_ok());
    }

    #[tokio::test]
    async fn test_task_submission() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create channels for testing
        let (task_sender, task_receiver) = flume::unbounded::<NotificationTask>();
        let (shutdown_sender, _shutdown_receiver) = flume::unbounded::<()>();

        // Start IPC server
        let queue_size = Arc::new(AtomicUsize::new(0));
        let server = IpcServer::new(
            socket_path.clone(),
            task_sender,
            shutdown_sender,
            queue_size,
        ).await.unwrap();

        let server_handle = tokio::spawn(async move {
            server.run().await.unwrap();
        });

        // Give server time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create client and submit task
        let client = IpcClient::new(socket_path);
        let task = NotificationTask {
            hook_name: "test-hook".to_string(),
            hook_data: serde_json::to_string(&serde_json::json!({"test": "data"})).unwrap(),
            retry_count: 0,
            timestamp: chrono::Local::now(),
            ntfy_config: crate::daemon::shared::NtfyTaskConfig {
                server_url: "https://ntfy.sh".to_string(),
                topic: "test-topic".to_string(),
                priority: Some(3),
                tags: Some(vec!["test".to_string()]),
                auth_token: None,
                send_format: "json".to_string(),
            },
            project_path: Some("/tmp/test-project".to_string()),
        };

        let submit_result = timeout(Duration::from_secs(1), client.send_task(task)).await;
        assert!(submit_result.is_ok() && submit_result.unwrap().is_ok());

        // Verify task was received
        let received_task = timeout(Duration::from_millis(500), task_receiver.recv_async()).await;
        assert!(received_task.is_ok());
        let received_task = received_task.unwrap().unwrap();
        assert_eq!(received_task.hook_name, "test-hook");

        // Shutdown server
        let _ = client.shutdown().await;
        let _ = timeout(Duration::from_secs(2), server_handle).await;
    }
}