use anyhow::{Context, Result};

use super::traits::{NotificationClient, RetryConfig};
use super::ntfy::{AsyncNtfyClient, NtfyClient, NtfyClientConfig};
use crate::config::{Config, NtfyConfig};

/// Client factory trait for dependency injection and testability
pub trait ClientFactory: Send + Sync {
    /// Create an async ntfy client
    fn create_async_ntfy_client(&self, config: &NtfyConfig) -> Result<Box<dyn NotificationClient>>;
    
    /// Create a sync ntfy client  
    fn create_sync_ntfy_client(&self, config: &NtfyConfig) -> Result<NtfyClient>;
    
    /// Create a client from a full application config
    fn create_from_app_config(&self, config: &Config) -> Result<Box<dyn NotificationClient>>;
    
    /// Get supported client types
    fn supported_types(&self) -> Vec<&'static str>;
}

/// Default implementation of ClientFactory
pub struct DefaultClientFactory {
    // Configuration overrides or customizations can be stored here
    default_timeout: Option<u64>,
    default_user_agent: Option<String>,
}

impl DefaultClientFactory {
    /// Create a new default client factory
    pub fn new() -> Self {
        Self {
            default_timeout: None,
            default_user_agent: Some("claude-ntfy/0.1.0".to_string()),
        }
    }
    
    /// Create a client factory with custom defaults
    pub fn with_defaults(timeout_secs: u64, user_agent: String) -> Self {
        Self {
            default_timeout: Some(timeout_secs),
            default_user_agent: Some(user_agent),
        }
    }
    
    /// Convert ntfy config to client config with factory defaults
    fn build_client_config(&self, config: &NtfyConfig) -> NtfyClientConfig {
        let mut client_config = NtfyClientConfig {
            server_url: config.server_url.clone(),
            auth_token: config.auth_token.clone(),
            timeout_secs: config.timeout_secs.or(self.default_timeout),
            send_format: config.send_format.clone(),
            user_agent: self.default_user_agent.clone(),
            ..Default::default()
        };
        
        // Apply any factory-level configuration optimizations
        self.optimize_client_config(&mut client_config);
        
        client_config
    }
    
    /// Apply factory-level optimizations to client configuration
    fn optimize_client_config(&self, config: &mut NtfyClientConfig) {
        // Set reasonable retry defaults based on send format
        if config.send_format == "json" {
            // JSON requests might benefit from more aggressive retries
            config.retry_config.max_attempts = 3;
            config.retry_config.base_delay_ms = 150;
        } else {
            // Text requests are simpler, fewer retries needed
            config.retry_config.max_attempts = 2;
            config.retry_config.base_delay_ms = 100;
        }
        
        // Optimize timeout based on server URL
        if config.server_url.contains("localhost") || config.server_url.contains("127.0.0.1") {
            // Local server, shorter timeout
            config.timeout_secs = Some(config.timeout_secs.unwrap_or(10).min(10));
        } else if config.server_url.contains("ntfy.sh") {
            // Public ntfy.sh service, reasonable timeout
            config.timeout_secs = Some(config.timeout_secs.unwrap_or(30));
        }
    }
}

impl Default for DefaultClientFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientFactory for DefaultClientFactory {
    fn create_async_ntfy_client(&self, config: &NtfyConfig) -> Result<Box<dyn NotificationClient>> {
        let client_config = self.build_client_config(config);
        let client = AsyncNtfyClient::new(client_config)
            .context("Failed to create async ntfy client")?;
        Ok(Box::new(client))
    }
    
    fn create_sync_ntfy_client(&self, config: &NtfyConfig) -> Result<NtfyClient> {
        let client_config = self.build_client_config(config);
        
        // Create async client first
        let async_client = AsyncNtfyClient::new(client_config)
            .context("Failed to create async ntfy client for sync wrapper")?;
            
        // Wrap in sync client
        Ok(async_client.blocking())
    }
    
    fn create_from_app_config(&self, config: &Config) -> Result<Box<dyn NotificationClient>> {
        self.create_async_ntfy_client(&config.ntfy)
    }
    
    fn supported_types(&self) -> Vec<&'static str> {
        vec!["ntfy", "ntfy-async", "ntfy-sync"]
    }
}

/// Mock client factory for testing
#[cfg(test)]
pub struct MockClientFactory {
    should_fail: bool,
    custom_config: Option<NtfyClientConfig>,
}

#[cfg(test)]
impl MockClientFactory {
    pub fn new() -> Self {
        Self {
            should_fail: false,
            custom_config: None,
        }
    }
    
    pub fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }
    
    pub fn with_custom_config(mut self, config: NtfyClientConfig) -> Self {
        self.custom_config = Some(config);
        self
    }
}

#[cfg(test)]
impl ClientFactory for MockClientFactory {
    fn create_async_ntfy_client(&self, _config: &NtfyConfig) -> Result<Box<dyn NotificationClient>> {
        if self.should_fail {
            anyhow::bail!("Mock factory configured to fail");
        }
        
        let client_config = self.custom_config.clone().unwrap_or_default();
        let client = AsyncNtfyClient::new(client_config)?;
        Ok(Box::new(client))
    }
    
    fn create_sync_ntfy_client(&self, config: &NtfyConfig) -> Result<NtfyClient> {
        if self.should_fail {
            anyhow::bail!("Mock factory configured to fail");
        }
        
        let async_client = self.create_async_ntfy_client(config)?;
        
        // This is a bit of a hack for testing, but works for our mock scenario
        let client_config = self.custom_config.clone().unwrap_or_default();
        let async_client = AsyncNtfyClient::new(client_config)?;
        Ok(async_client.blocking())
    }
    
    fn create_from_app_config(&self, config: &Config) -> Result<Box<dyn NotificationClient>> {
        self.create_async_ntfy_client(&config.ntfy)
    }
    
    fn supported_types(&self) -> Vec<&'static str> {
        vec!["mock-ntfy"]
    }
}

/// Convenience functions for quick client creation
pub mod convenience {
    use super::*;
    
    /// Create a default async ntfy client from a server URL
    pub fn create_async_client(server_url: &str, auth_token: Option<String>) -> Result<AsyncNtfyClient> {
        let config = NtfyClientConfig {
            server_url: server_url.to_string(),
            auth_token,
            ..Default::default()
        };
        AsyncNtfyClient::new(config)
    }
    
    /// Create a default sync ntfy client from a server URL
    pub fn create_sync_client(server_url: &str, auth_token: Option<String>) -> Result<NtfyClient> {
        let async_client = create_async_client(server_url, auth_token)?;
        Ok(async_client.blocking())
    }
    
    /// Create a client with all default settings for ntfy.sh
    pub fn create_default_client() -> Result<AsyncNtfyClient> {
        create_async_client("https://ntfy.sh", None)
    }
    
    /// Create a high-performance client with optimized settings
    pub fn create_high_performance_client(server_url: &str, auth_token: Option<String>) -> Result<AsyncNtfyClient> {
        let config = NtfyClientConfig {
            server_url: server_url.to_string(),
            auth_token,
            timeout_secs: Some(15), // Faster timeout
            send_format: "json".to_string(), // JSON is generally faster to process
            retry_config: RetryConfig {
                max_attempts: 2, // Fewer retries for speed
                base_delay_ms: 50, // Faster retries
                ..Default::default()
            },
            user_agent: Some("claude-ntfy-fast/0.1.0".to_string()),
        };
        AsyncNtfyClient::new(config)
    }
    
    /// Create a reliable client with extensive retry logic
    pub fn create_reliable_client(server_url: &str, auth_token: Option<String>) -> Result<AsyncNtfyClient> {
        let config = NtfyClientConfig {
            server_url: server_url.to_string(),
            auth_token,
            timeout_secs: Some(60), // Longer timeout
            send_format: "json".to_string(),
            retry_config: RetryConfig {
                max_attempts: 5, // More retries
                base_delay_ms: 200, // Longer delays
                max_delay_ms: 10000, // Allow longer delays
                backoff_multiplier: 1.5, // Gentler backoff
                jitter_factor: 0.2, // More jitter
            },
            user_agent: Some("claude-ntfy-reliable/0.1.0".to_string()),
        };
        AsyncNtfyClient::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NtfyConfig;
    
    #[tokio::test]
    async fn test_default_factory_creation() {
        let factory = DefaultClientFactory::new();
        let supported = factory.supported_types();
        assert!(supported.contains(&"ntfy"));
        assert!(supported.contains(&"ntfy-async"));
        assert!(supported.contains(&"ntfy-sync"));
    }
    
    #[tokio::test]
    async fn test_client_creation_from_config() {
        let factory = DefaultClientFactory::new();
        let config = NtfyConfig {
            server_url: "https://ntfy.example.com".to_string(),
            auth_token: Some("test-token".to_string()),
            timeout_secs: Some(45),
            send_format: "text".to_string(),
            ..Default::default()
        };
        
        let client = factory.create_async_ntfy_client(&config);
        assert!(client.is_ok());
    }
    
    #[tokio::test]
    async fn test_convenience_functions() {
        let client = convenience::create_default_client();
        assert!(client.is_ok());
        
        let sync_client = convenience::create_sync_client("https://ntfy.sh", None);
        assert!(sync_client.is_ok());
    }
    
    #[tokio::test]
    async fn test_performance_client_config() {
        let client = convenience::create_high_performance_client("https://fast.ntfy.example.com", None);
        assert!(client.is_ok());
        
        let client = client.unwrap();
        let config_info = client.get_config_info();
        assert_eq!(config_info.timeout_secs, 15);
        assert_eq!(config_info.max_retries, 2);
    }
    
    #[tokio::test]
    async fn test_mock_factory() {
        let factory = MockClientFactory::new();
        let config = NtfyConfig {
            server_url: "https://ntfy.sh".to_string(),
            default_topic: "test".to_string(),
            send_format: "json".to_string(),
            ..Default::default()
        };
        let client = factory.create_async_ntfy_client(&config);
        assert!(client.is_ok());
        
        let failing_factory = MockClientFactory::new().with_failure();
        let client = failing_factory.create_async_ntfy_client(&config);
        assert!(client.is_err());
    }
}