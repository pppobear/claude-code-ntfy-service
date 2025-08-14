use async_trait::async_trait;
use anyhow::Result;
use std::time::Duration;

use super::super::ntfy::NtfyMessage;

/// Unified notification client interface for async operations
#[async_trait]
pub trait NotificationClient: Send + Sync {
    /// Send a notification message
    async fn send(&self, message: &NtfyMessage) -> Result<()>;
    
    /// Validate client configuration (connectivity, auth, etc.)
    async fn validate_config(&self) -> Result<()>;
    
    /// Perform health check against the service
    async fn health_check(&self) -> Result<()>;
    
    /// Get client performance statistics
    fn get_stats(&self) -> ClientStats;
    
    /// Get client configuration info
    fn get_config_info(&self) -> ClientConfigInfo;
}

/// Performance and usage statistics for notification clients
#[derive(Debug, Clone)]
pub struct ClientStats {
    /// Total number of messages successfully sent
    pub messages_sent: u64,
    /// Total number of failed message attempts
    pub messages_failed: u64,
    /// Average latency in milliseconds
    pub average_latency_ms: u64,
    /// Minimum recorded latency
    pub min_latency_ms: u64,
    /// Maximum recorded latency  
    pub max_latency_ms: u64,
    /// Last error encountered (if any)
    pub last_error: Option<String>,
    /// Total number of retry attempts made
    pub retry_attempts: u64,
    /// Client uptime duration
    pub uptime: Duration,
}

impl Default for ClientStats {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_failed: 0,
            average_latency_ms: 0,
            min_latency_ms: u64::MAX,
            max_latency_ms: 0,
            last_error: None,
            retry_attempts: 0,
            uptime: Duration::new(0, 0),
        }
    }
}

impl ClientStats {
    /// Update statistics with a successful send operation
    pub fn record_success(&mut self, latency_ms: u64) {
        self.messages_sent += 1;
        self.average_latency_ms = if self.messages_sent == 1 {
            latency_ms
        } else {
            (self.average_latency_ms + latency_ms) / 2
        };
        self.min_latency_ms = self.min_latency_ms.min(latency_ms);
        self.max_latency_ms = self.max_latency_ms.max(latency_ms);
    }
    
    /// Update statistics with a failed send operation
    pub fn record_failure(&mut self, error: String) {
        self.messages_failed += 1;
        self.last_error = Some(error);
    }
    
    /// Record a retry attempt
    pub fn record_retry(&mut self) {
        self.retry_attempts += 1;
    }
    
    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        let total = self.messages_sent + self.messages_failed;
        if total == 0 {
            0.0
        } else {
            (self.messages_sent as f64 / total as f64) * 100.0
        }
    }
}

/// Configuration information for a notification client
#[derive(Debug, Clone)]
pub struct ClientConfigInfo {
    /// Server URL being used
    pub server_url: String,
    /// Whether authentication is configured
    pub has_auth: bool,
    /// Send format preference (text/json)
    pub send_format: String,
    /// Configured timeout in seconds
    pub timeout_secs: u64,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry delay in milliseconds
    pub retry_delay_ms: u64,
}

/// Retry configuration for notification clients
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Base delay between retries in milliseconds
    pub base_delay_ms: u64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier (exponential backoff)
    pub backoff_multiplier: f64,
    /// Jitter factor to add randomness to retry delays
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a specific retry attempt
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.base_delay_ms as f64;
        let delay = base_delay * self.backoff_multiplier.powi(attempt as i32);
        let delay = delay.min(self.max_delay_ms as f64);
        
        // Add jitter
        let jitter = delay * self.jitter_factor * (rand::random::<f64>() - 0.5);
        let final_delay = (delay + jitter).max(0.0) as u64;
        
        Duration::from_millis(final_delay)
    }
    
    /// Create a retry config with exponential backoff
    pub fn exponential(max_attempts: u32, base_delay_ms: u64) -> Self {
        Self {
            max_attempts,
            base_delay_ms,
            backoff_multiplier: 2.0,
            ..Default::default()
        }
    }
    
    /// Create a retry config with linear backoff
    pub fn linear(max_attempts: u32, delay_ms: u64) -> Self {
        Self {
            max_attempts,
            base_delay_ms: delay_ms,
            max_delay_ms: delay_ms,
            backoff_multiplier: 1.0,
            jitter_factor: 0.0,
        }
    }
}