//! IPC Server for daemon communication
//!
//! This module provides a Unix socket server for handling daemon IPC communication.

use anyhow::{Context, Result};
use flume::{Receiver, Sender};
use std::sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info, warn};

use super::shared::{DaemonMessage, DaemonResponse, NotificationTask};

/// IPC server for handling daemon communication
pub struct IpcServer {
    listener: UnixListener,
    task_sender: Sender<NotificationTask>,
    shutdown_receiver: Receiver<()>,
    shutdown_sender: Sender<()>,
    main_shutdown_sender: Sender<()>,
    queue_size: Arc<AtomicUsize>,
    is_running: Arc<AtomicBool>,
    start_time: std::time::Instant,
}

impl IpcServer {
    /// Create new IPC server
    pub fn new(
        socket_path: &std::path::Path,
        task_sender: Sender<NotificationTask>,
        shutdown_receiver: Receiver<()>,
        shutdown_sender: Sender<()>,
        queue_size: Arc<AtomicUsize>,
        main_shutdown_sender: Sender<()>,
    ) -> Result<Self> {
        // Remove existing socket file if it exists
        if socket_path.exists() {
            std::fs::remove_file(socket_path)
                .context("Failed to remove existing socket file")?;
        }

        // Create socket listener
        let listener = UnixListener::bind(socket_path)
            .context("Failed to bind Unix socket")?;

        info!("IPC server bound to socket: {}", socket_path.display());

        Ok(IpcServer {
            listener,
            task_sender,
            shutdown_receiver,
            shutdown_sender,
            main_shutdown_sender,
            queue_size,
            is_running: Arc::new(AtomicBool::new(true)),
            start_time: std::time::Instant::now(),
        })
    }

    /// Run the IPC server
    pub async fn run(self) -> Result<()> {
        info!("IPC server started");

        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = self.shutdown_receiver.recv_async() => {
                    info!("IPC server received external shutdown signal");
                    break;
                }

                // Handle incoming connections
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            debug!("New IPC client connection");
                            let task_sender = self.task_sender.clone();
                            let shutdown_sender = self.shutdown_sender.clone();
                            let main_shutdown_sender = self.main_shutdown_sender.clone();
                            let queue_size = self.queue_size.clone();
                            let is_running = self.is_running.clone();
                            let start_time = self.start_time;

                            tokio::spawn(async move {
                                if let Err(e) = Self::handle_client(
                                    stream, task_sender, shutdown_sender, main_shutdown_sender, queue_size, is_running, start_time
                                ).await {
                                    error!("Error handling IPC client: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept IPC connection: {}", e);
                        }
                    }
                }
            }
        }

        self.is_running.store(false, Ordering::Relaxed);
        info!("IPC server stopped");
        Ok(())
    }

    /// Handle individual client connection
    async fn handle_client(
        mut stream: UnixStream,
        task_sender: Sender<NotificationTask>,
        shutdown_sender: Sender<()>,
        main_shutdown_sender: Sender<()>,
        queue_size: Arc<AtomicUsize>,
        is_running: Arc<AtomicBool>,
        start_time: std::time::Instant,
    ) -> Result<()> {
        // Read message length
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await
            .context("Failed to read message length")?;

        let message_length = u32::from_le_bytes(length_bytes) as usize;

        // Validate message length
        if message_length > 1024 * 1024 { // 1MB max message
            return Err(anyhow::anyhow!("Message too large: {} bytes", message_length));
        }

        // Read message payload
        let mut message_buffer = vec![0u8; message_length];
        stream.read_exact(&mut message_buffer).await
            .context("Failed to read message payload")?;

        // Deserialize message
        let (message, _): (DaemonMessage, usize) = bincode::serde::decode_from_slice(&message_buffer, bincode::config::standard())
            .context("Failed to deserialize message")?;

        debug!("Received IPC message: {:?}", message);

        // Process message and generate response
        let response = match message {
            DaemonMessage::Submit(task) => {
                // Increment queue size when task is queued
                queue_size.fetch_add(1, Ordering::Relaxed);
                
                match task_sender.send_async(*task).await {
                    Ok(()) => DaemonResponse::Ok,
                    Err(e) => {
                        // Decrement on failure
                        queue_size.fetch_sub(1, Ordering::Relaxed);
                        DaemonResponse::Error(format!("Failed to queue task: {e}"))
                    }
                }
            }
            DaemonMessage::Status => {
                let uptime_secs = start_time.elapsed().as_secs();
                let current_queue_size = queue_size.load(Ordering::Relaxed);
                let running = is_running.load(Ordering::Relaxed);

                DaemonResponse::Status {
                    queue_size: current_queue_size,
                    is_running: running,
                    uptime_secs,
                }
            }
            DaemonMessage::Shutdown => {
                info!("Received shutdown request via IPC");
                if let Err(e) = shutdown_sender.send_async(()).await {
                    warn!("Failed to send shutdown signal to notification daemon: {}", e);
                }
                if let Err(e) = main_shutdown_sender.send_async(()).await {
                    warn!("Failed to send shutdown signal to main process: {}", e);
                }
                DaemonResponse::Ok
            }
            DaemonMessage::Reload => {
                // For now, just acknowledge reload
                info!("Received reload request via IPC");
                DaemonResponse::Ok
            }
            DaemonMessage::Ping => {
                DaemonResponse::Ok
            }
        };

        // Serialize and send response
        let response_data = bincode::serde::encode_to_vec(&response, bincode::config::standard())
            .context("Failed to serialize response")?;

        let response_length = response_data.len() as u32;
        let response_length_bytes = response_length.to_le_bytes();

        // Send response length
        stream.write_all(&response_length_bytes).await
            .context("Failed to write response length")?;

        // Send response payload
        stream.write_all(&response_data).await
            .context("Failed to write response payload")?;

        stream.flush().await
            .context("Failed to flush response")?;

        debug!("Sent IPC response: {:?}", response);
        Ok(())
    }
}