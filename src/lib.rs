//! Claude Code Ntfy Service Library
//! 
//! This library provides the core functionality for integrating Claude Code hooks
//! with ntfy notification services.

pub mod config;
pub mod daemon;
pub mod daemon_shared;
pub mod hooks;
pub mod templates;
pub mod errors;
pub mod shared;
pub mod ntfy;

// Re-export commonly used types for convenience
pub use config::{Config, ConfigManager, NtfyConfig};
pub use hooks::{ProcessedHook, DefaultHookProcessor, create_default_processor};