//! Shared types and utilities for daemon communication
//!
//! This module contains types shared between the daemon server and IPC client.

use serde::{Deserialize, Serialize};

/// Notification task structure for daemon processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTask {
    pub hook_name: String,
    pub hook_data: String, // Store as JSON string for bincode compatibility
    pub retry_count: u32,
    pub timestamp: chrono::DateTime<chrono::Local>,
}

/// IPC message types for daemon communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonMessage {
    Submit(NotificationTask),
    Ping,
    Shutdown,
    Reload,
    Status,
}

/// Daemon response types
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