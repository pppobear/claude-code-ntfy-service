//! IPC utilities for daemon communication
//!
//! This module provides utilities for daemon socket path creation.

use anyhow::{Context, Result};
use std::path::PathBuf;

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