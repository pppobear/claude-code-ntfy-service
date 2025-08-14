//! Daemon module for background notification processing
//!
//! This module provides:
//! - High-performance Unix socket IPC communication
//! - Async notification processing
//! - Background daemon server
//! - Client interface for CLI communication

pub mod ipc;
pub mod server;
pub mod shared;

// Re-export commonly used types
pub use ipc::{IpcClient, IpcServer};
pub use server::NotificationDaemon;
pub use shared::{DaemonMessage, DaemonResponse, NotificationTask};

// Re-export utilities for backward compatibility
pub use ipc::create_socket_path;