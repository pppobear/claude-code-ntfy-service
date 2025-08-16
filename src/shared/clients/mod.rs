//! Unified clients module for notification services
//!
//! This module provides a unified, async-first approach to notification clients
//! with modern async patterns. It eliminates code duplication between
//! sync and async implementations while providing clean interfaces.
//!
//! ## Architecture
//!
//! - **AsyncNtfyClient**: Primary async-first implementation with advanced features
//! - **NtfyClient**: Sync wrapper around AsyncNtfyClient for blocking operations  
//! - **Traits**: Clean interfaces with comprehensive error handling
//!
//! ## Features
//!
//! - **Unified Implementation**: Single codebase eliminates sync/async duplication
//! - **Retry Logic**: Configurable exponential backoff with jitter
//! - **Statistics**: Performance tracking and health monitoring
//! - **Type Safety**: Strong typing with comprehensive error context
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use claude_ntfy::shared::clients::ntfy::{AsyncNtfyClient, NtfyClientConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // Create a client with configuration  
//! let config = NtfyClientConfig {
//!     server_url: "https://my-ntfy.example.com".to_string(),
//!     auth_token: Some("my-token".to_string()),
//!     ..Default::default()
//! };
//! let client = AsyncNtfyClient::new(config)?;
//!
//! // Send a notification
//! client.send_simple("topic", "Title", "Message", 3).await?;
//! # Ok(())
//! # }
//! ```

pub mod ntfy;
pub mod traits;

// Re-export main types for convenience
pub use ntfy::{AsyncNtfyClient, create_async_client_from_ntfy_config, create_sync_client_from_ntfy_config};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NtfyConfig;
    
    fn create_test_ntfy_config() -> NtfyConfig {
        NtfyConfig {
            server_url: "https://ntfy.example.com".to_string(),
            auth_token: Some("test-token".to_string()),
            timeout_secs: Some(30),
            send_format: "json".to_string(),
            ..Default::default()
        }
    }
    
    #[tokio::test]
    async fn test_create_async_client_from_config() {
        let config = create_test_ntfy_config();
        let client = create_async_client_from_ntfy_config(&config);
        assert!(client.is_ok());
    }
    
    #[tokio::test]
    async fn test_create_sync_client_from_config() {
        let config = create_test_ntfy_config();
        let client = create_sync_client_from_ntfy_config(&config);
        assert!(client.is_ok());
    }
}