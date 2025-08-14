//! Shared daemon types and utilities
//! 
//! This module contains types and utilities shared between the main CLI
//! application and the daemon process to avoid code duplication.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// Create socket path for daemon communication
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

/// Check if a process is running by PID
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_socket_path_with_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();
        
        let socket_path = create_socket_path(Some(&project_path)).unwrap();
        
        assert!(socket_path.parent().unwrap().exists());
        assert_eq!(socket_path.file_name().unwrap(), "daemon.sock");
        assert!(socket_path.to_string_lossy().contains(".claude/ntfy-service"));
    }

    #[test]
    fn test_create_socket_path_global() {
        let socket_path = create_socket_path(None).unwrap();
        
        assert!(socket_path.parent().unwrap().exists());
        assert_eq!(socket_path.file_name().unwrap(), "daemon.sock");
        assert!(socket_path.to_string_lossy().contains(".claude/ntfy-service"));
    }

    #[test]
    fn test_daemon_message_serialization() {
        let task = NotificationTask {
            hook_name: "test".to_string(),
            hook_data: serde_json::json!({"test": "data"}),
            retry_count: 0,
            timestamp: chrono::Local::now(),
        };

        let message = DaemonMessage::Submit(task);
        let serialized = serde_json::to_string(&message).unwrap();
        let deserialized: DaemonMessage = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            DaemonMessage::Submit(deserialized_task) => {
                assert_eq!(deserialized_task.hook_name, "test");
            }
            _ => panic!("Expected Submit message"),
        }
    }
}