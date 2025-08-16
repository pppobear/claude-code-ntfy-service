//! IPC (Inter-Process Communication) client module
//! 
//! This module provides a unified interface for communicating with the daemon
//! via Unix domain sockets, reducing code duplication across handlers.

use crate::daemon::{DaemonMessage, DaemonResponse};
use anyhow::{Context, Result};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing::debug;

/// Configuration for IPC client behavior
#[derive(Debug, Clone)]
pub struct IpcClientConfig {
    /// Maximum allowed response size in bytes
    pub max_response_size: usize,
}

impl Default for IpcClientConfig {
    fn default() -> Self {
        Self {
            max_response_size: 1024 * 1024, // 1MB default
        }
    }
}

impl IpcClientConfig {
    /// Create config optimized for small responses (like status checks)
    pub fn small_response() -> Self {
        Self {
            max_response_size: 1024, // 1KB
        }
    }
    
    /// Create config optimized for large responses (like detailed status)
    pub fn large_response() -> Self {
        Self {
            max_response_size: 1024 * 1024, // 1MB
        }
    }
}

/// Unified IPC client for daemon communication
pub struct IpcClient {
    config: IpcClientConfig,
}

impl IpcClient {
    /// Create a new IPC client with default configuration
    pub fn new() -> Self {
        Self {
            config: IpcClientConfig::default(),
        }
    }
    
    /// Create a new IPC client with custom configuration
    pub fn with_config(config: IpcClientConfig) -> Self {
        Self { config }
    }
    
    /// Send a message to daemon and expect a typed response
    pub async fn send_message<T>(&self, socket_path: &Path, message: DaemonMessage) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        debug!("Sending IPC message to daemon at {}", socket_path.display());
        
        // Connect to Unix socket
        let mut stream = UnixStream::connect(socket_path)
            .await
            .context("Failed to connect to daemon socket")?;

        // Serialize message
        let serialized = bincode::serde::encode_to_vec(&message, bincode::config::standard())
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

        debug!("Message sent, waiting for response");

        // Read response length
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await
            .context("Failed to read response length")?;

        let response_length = u32::from_le_bytes(length_bytes) as usize;

        // Validate response length
        if response_length > self.config.max_response_size {
            return Err(anyhow::anyhow!(
                "Response too large: {} bytes (max: {})",
                response_length,
                self.config.max_response_size
            ));
        }

        // Read response payload
        let mut response_buffer = vec![0u8; response_length];
        stream.read_exact(&mut response_buffer).await
            .context("Failed to read response payload")?;

        // Deserialize response
        let (response, _): (T, usize) = bincode::serde::decode_from_slice(&response_buffer, bincode::config::standard())
            .context("Failed to deserialize response")?;

        debug!("Received and deserialized response successfully");
        Ok(response)
    }
    
    /// Send a message to daemon and expect a DaemonResponse
    pub async fn send_daemon_message(&self, socket_path: &Path, message: DaemonMessage) -> Result<DaemonResponse> {
        self.send_message(socket_path, message).await
    }
    
    /// Send a message and only check for success (ignores response content)
    pub async fn send_fire_and_forget(&self, socket_path: &Path, message: DaemonMessage) -> Result<()> {
        let _response: DaemonResponse = self.send_message(socket_path, message).await?;
        Ok(())
    }
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for common IPC operations
pub mod convenience {
    use super::*;
    use crate::daemon::NotificationTask;
    
    /// Send a notification task to the daemon
    pub async fn send_notification_task(socket_path: &Path, task: NotificationTask) -> Result<()> {
        let client = IpcClient::with_config(IpcClientConfig::small_response());
        let message = DaemonMessage::Submit(Box::new(task));
        client.send_fire_and_forget(socket_path, message).await
    }
    
    /// Get daemon status
    pub async fn get_daemon_status(socket_path: &Path) -> Result<DaemonResponse> {
        let client = IpcClient::with_config(IpcClientConfig::large_response());
        client.send_daemon_message(socket_path, DaemonMessage::Status).await
    }
    
    /// Send shutdown signal to daemon
    pub async fn shutdown_daemon(socket_path: &Path) -> Result<DaemonResponse> {
        let client = IpcClient::with_config(IpcClientConfig::small_response());
        client.send_daemon_message(socket_path, DaemonMessage::Shutdown).await
    }
    
    /// Send reload signal to daemon  
    pub async fn reload_daemon(socket_path: &Path) -> Result<DaemonResponse> {
        let client = IpcClient::with_config(IpcClientConfig::small_response());
        client.send_daemon_message(socket_path, DaemonMessage::Reload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ipc_client_config_defaults() {
        let config = IpcClientConfig::default();
        assert_eq!(config.max_response_size, 1024 * 1024);
    }
    
    #[test]
    fn test_ipc_client_config_small_response() {
        let config = IpcClientConfig::small_response();
        assert_eq!(config.max_response_size, 1024);
    }
    
    #[test]
    fn test_ipc_client_config_large_response() {
        let config = IpcClientConfig::large_response();
        assert_eq!(config.max_response_size, 1024 * 1024);
    }
    
    #[test]
    fn test_ipc_client_creation() {
        let client = IpcClient::new();
        assert_eq!(client.config.max_response_size, 1024 * 1024);
        
        let custom_config = IpcClientConfig::small_response();
        let client_with_config = IpcClient::with_config(custom_config.clone());
        assert_eq!(client_with_config.config.max_response_size, custom_config.max_response_size);
    }
}