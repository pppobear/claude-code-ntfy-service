//! Shared types and utilities for daemon communication
//!
//! This module contains types shared between the daemon server and IPC client,
//! organized into logical groups for better maintainability.

use serde::{Deserialize, Serialize};

// =============================================================================
// Constants
// =============================================================================

/// Supported notification send formats
pub mod send_format {
    pub const TEXT: &str = "text";
    
    /// Default send format
    pub const DEFAULT: &str = TEXT;
}

/// Default values for configuration
pub mod defaults {
    pub const SERVER_URL: &str = "https://ntfy.sh";
    pub const TOPIC: &str = "claude-notifications";
    pub const PRIORITY: u8 = 3;
}

// =============================================================================
// Configuration Types
// =============================================================================

/// Ntfy configuration for a specific notification task
///
/// This structure contains all the necessary information to send a notification
/// to an ntfy server, including authentication and formatting preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NtfyTaskConfig {
    /// The ntfy server URL (e.g., "https://ntfy.sh")
    pub server_url: String,
    
    /// The notification topic name
    pub topic: String,
    
    /// Priority level (1-5, where 5 is highest)
    pub priority: Option<u8>,
    
    /// Optional tags to categorize the notification
    pub tags: Option<Vec<String>>,
    
    /// Authentication token for private topics
    pub auth_token: Option<String>,
    
    /// Message format: "text" or "json"
    pub send_format: String,
}

impl NtfyTaskConfig {
    /// Create a new configuration with minimum required fields
    pub fn new(server_url: impl Into<String>, topic: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            topic: topic.into(),
            priority: Some(defaults::PRIORITY),
            tags: None,
            auth_token: None,
            send_format: send_format::DEFAULT.to_string(),
        }
    }
    
    
    
}

impl Default for NtfyTaskConfig {
    fn default() -> Self {
        Self::new(defaults::SERVER_URL, defaults::TOPIC)
    }
}

// =============================================================================
// Task Types
// =============================================================================

/// Notification task structure for daemon processing
///
/// This represents a single notification task that needs to be processed by the daemon.
/// It includes all necessary metadata for processing, retry logic, and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTask {
    /// The Claude Code hook name that triggered this notification
    pub hook_name: String,
    
    /// Hook data as JSON string (for bincode compatibility)
    pub hook_data: String,
    
    /// Number of retry attempts made
    pub retry_count: u32,
    
    /// Timestamp when the task was created
    pub timestamp: chrono::DateTime<chrono::Local>,
    
    /// Ntfy configuration for this specific task
    pub ntfy_config: NtfyTaskConfig,
    
    /// Source project path (for logging and debugging)
    pub project_path: Option<String>,
}

impl NotificationTask {
}

// =============================================================================
// Communication Types
// =============================================================================

/// IPC message types for daemon communication
///
/// These messages are sent from clients to the daemon via Unix socket IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonMessage {
    /// Submit a notification task for processing
    Submit(Box<NotificationTask>),
    
    /// Ping the daemon to check if it's alive
    Ping,
    
    /// Request daemon shutdown
    Shutdown,
    
    /// Request daemon configuration reload
    Reload,
    
    /// Request daemon status information
    Status,
}


/// Daemon response types
///
/// These responses are sent back to clients after processing their requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonResponse {
    /// Operation completed successfully
    Ok,
    
    /// Operation failed with error message
    Error(String),
    
    /// Status information response
    Status {
        /// Number of tasks in the queue
        queue_size: usize,
        /// Whether the daemon is running
        is_running: bool,
        /// Daemon uptime in seconds
        uptime_secs: u64,
    },
}



