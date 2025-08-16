//! Daemon module for background notification processing
//!
//! This module provides:
//! - High-performance Unix socket IPC communication
//! - Async notification processing
//! - Background daemon server
//! - Client interface for CLI communication

pub mod ipc;
pub mod ipc_server;
pub mod server;
pub mod shared;

// Re-export commonly used types
pub use shared::{DaemonMessage, DaemonResponse, NotificationTask, NtfyTaskConfig};

// Re-export utilities for backward compatibility
pub use ipc::create_socket_path;
pub use server::is_process_running;