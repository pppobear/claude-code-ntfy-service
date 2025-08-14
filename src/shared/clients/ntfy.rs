use async_trait::async_trait;
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE}};
use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use url::Url;
use tokio::time::sleep;

use super::traits::{NotificationClient, ClientStats, ClientConfigInfo, RetryConfig};
use crate::ntfy::NtfyMessage;

/// Configuration for the ntfy client
#[derive(Debug, Clone)]
pub struct NtfyClientConfig {
    pub server_url: String,
    pub auth_token: Option<String>,
    pub timeout_secs: Option<u64>,
    pub send_format: String,
    pub retry_config: RetryConfig,
    pub user_agent: Option<String>,
}

impl Default for NtfyClientConfig {
    fn default() -> Self {
        Self {
            server_url: "https://ntfy.sh".to_string(),
            auth_token: None,
            timeout_secs: Some(30),
            send_format: "json".to_string(),
            retry_config: RetryConfig::default(),
            user_agent: Some("claude-ntfy/0.1.0".to_string()),
        }
    }
}

/// Primary async-first ntfy client implementation
#[derive(Clone)]
pub struct AsyncNtfyClient {
    client: Client,
    config: NtfyClientConfig,
    stats: Arc<Mutex<ClientStats>>,
    created_at: Instant,
}

impl AsyncNtfyClient {
    /// Create a new async ntfy client with configuration
    pub fn new(config: NtfyClientConfig) -> Result<Self> {
        let timeout = Duration::from_secs(config.timeout_secs.unwrap_or(30));
        
        let mut client_builder = Client::builder()
            .timeout(timeout)
            .tcp_keepalive(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10);
            
        // Set user agent if provided
        if let Some(user_agent) = &config.user_agent {
            client_builder = client_builder.user_agent(user_agent);
        }
        
        let client = client_builder
            .build()
            .context("Failed to create async HTTP client")?;
        
        let created_at = Instant::now();
        let stats = Arc::new(Mutex::new(ClientStats::default()));
        
        Ok(Self {
            client,
            config,
            stats,
            created_at,
        })
    }
    
    /// Create a sync wrapper around this async client
    pub fn blocking(self) -> NtfyClient {
        NtfyClient::new(self)
    }
    
    /// Send a notification with built-in retry logic
    async fn send_with_retry(&self, message: &NtfyMessage) -> Result<()> {
        let mut last_error = None;
        
        for attempt in 0..=self.config.retry_config.max_attempts {
            match self.send_internal(message).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    
                    if attempt < self.config.retry_config.max_attempts {
                        // Record retry attempt
                        if let Ok(mut stats) = self.stats.lock() {
                            stats.record_retry();
                        }
                        
                        // Calculate delay and wait
                        let delay = self.config.retry_config.calculate_delay(attempt);
                        sleep(delay).await;
                    }
                }
            }
        }
        
        // All retries exhausted
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Send failed after retries")))
    }
    
    /// Internal send implementation without retry logic
    async fn send_internal(&self, message: &NtfyMessage) -> Result<()> {
        let url = self.build_url(&message.topic)?;
        let headers = self.build_headers()?;
        
        let response = if self.config.send_format == "json" {
            self.send_json(url, headers, message).await?
        } else {
            self.send_text(url, headers, message).await?
        };
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to send notification: {} - {}", status, error_text);
        }
        
        Ok(())
    }
    
    /// Send notification as JSON
    async fn send_json(&self, url: String, mut headers: HeaderMap, message: &NtfyMessage) -> Result<reqwest::Response> {
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let body = self.build_json_body(message)?;
        
        self.client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .context("Failed to send JSON notification")
    }
    
    /// Send notification as plain text with headers
    async fn send_text(&self, url: String, mut headers: HeaderMap, message: &NtfyMessage) -> Result<reqwest::Response> {
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"));
        
        // Add ntfy-specific headers
        self.add_ntfy_headers(&mut headers, message)?;
        
        self.client
            .post(&url)
            .headers(headers)
            .body(message.message.clone())
            .send()
            .await
            .context("Failed to send text notification")
    }
    
    /// Build common headers (auth, etc.)
    fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        
        // Add authorization if configured
        if let Some(token) = &self.config.auth_token {
            let auth_value = format!("Bearer {}", token);
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth_value).context("Invalid auth token")?,
            );
        }
        
        Ok(headers)
    }
    
    /// Add ntfy-specific headers for text mode
    fn add_ntfy_headers(&self, headers: &mut HeaderMap, message: &NtfyMessage) -> Result<()> {
        if let Some(title) = &message.title {
            headers.insert(
                "X-Title",
                HeaderValue::from_str(title).context("Invalid title value")?,
            );
        }
        
        if let Some(priority) = message.priority {
            headers.insert(
                "X-Priority",
                HeaderValue::from_str(&priority.to_string())
                    .context("Invalid priority value")?,
            );
        }
        
        if let Some(tags) = &message.tags {
            let tags_str = tags.join(",");
            headers.insert(
                "X-Tags",
                HeaderValue::from_str(&tags_str).context("Invalid tags value")?,
            );
        }
        
        // Add other optional headers
        if let Some(click) = &message.click {
            headers.insert("X-Click", HeaderValue::from_str(click)?);
        }
        
        if let Some(attach) = &message.attach {
            headers.insert("X-Attach", HeaderValue::from_str(attach)?);
        }
        
        if let Some(delay) = &message.delay {
            headers.insert("X-Delay", HeaderValue::from_str(delay)?);
        }
        
        if let Some(email) = &message.email {
            headers.insert("X-Email", HeaderValue::from_str(email)?);
        }
        
        if let Some(call) = &message.call {
            headers.insert("X-Call", HeaderValue::from_str(call)?);
        }
        
        Ok(())
    }
    
    /// Build URL for the given topic
    fn build_url(&self, topic: &str) -> Result<String> {
        let base = Url::parse(&self.config.server_url).context("Invalid base URL")?;
        let url = base.join(topic).context("Failed to build topic URL")?;
        Ok(url.to_string())
    }
    
    /// Build JSON body for the message
    fn build_json_body(&self, message: &NtfyMessage) -> Result<serde_json::Value> {
        let mut body = serde_json::json!({
            "message": message.message,
        });
        
        if let Some(title) = &message.title {
            body["title"] = serde_json::json!(title);
        }
        
        if let Some(priority) = message.priority {
            body["priority"] = serde_json::json!(priority);
        }
        
        if let Some(tags) = &message.tags {
            body["tags"] = serde_json::json!(tags);
        }
        
        if let Some(click) = &message.click {
            body["click"] = serde_json::json!(click);
        }
        
        if let Some(attach) = &message.attach {
            body["attach"] = serde_json::json!(attach);
        }
        
        if let Some(filename) = &message.filename {
            body["filename"] = serde_json::json!(filename);
        }
        
        if let Some(delay) = &message.delay {
            body["delay"] = serde_json::json!(delay);
        }
        
        if let Some(email) = &message.email {
            body["email"] = serde_json::json!(email);
        }
        
        if let Some(call) = &message.call {
            body["call"] = serde_json::json!(call);
        }
        
        if let Some(actions) = &message.actions {
            body["actions"] = serde_json::json!(actions);
        }
        
        Ok(body)
    }
    
    /// Simple convenience method for sending basic notifications
    pub async fn send_simple(&self, topic: &str, title: &str, message: &str, priority: u8) -> Result<()> {
        let msg = NtfyMessage {
            topic: topic.to_string(),
            title: Some(title.to_string()),
            message: message.to_string(),
            priority: Some(priority),
            tags: None,
            click: None,
            attach: None,
            filename: None,
            delay: None,
            email: None,
            call: None,
            actions: None,
        };
        
        self.send(&msg).await
    }
}

#[async_trait]
impl NotificationClient for AsyncNtfyClient {
    async fn send(&self, message: &NtfyMessage) -> Result<()> {
        let start = Instant::now();
        
        let result = self.send_with_retry(message).await;
        
        let elapsed = start.elapsed().as_millis() as u64;
        
        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            match &result {
                Ok(_) => stats.record_success(elapsed),
                Err(e) => stats.record_failure(e.to_string()),
            }
        }
        
        result
    }
    
    async fn validate_config(&self) -> Result<()> {
        // Validate server URL format
        Url::parse(&self.config.server_url).context("Invalid server URL")?;
        
        // Test connectivity with a minimal health check
        self.health_check().await
    }
    
    async fn health_check(&self) -> Result<()> {
        let health_url = format!("{}/v1/health", self.config.server_url);
        
        let response = self.client
            .get(&health_url)
            .send()
            .await
            .context("Health check request failed")?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            anyhow::bail!("Health check failed: HTTP {}", response.status())
        }
    }
    
    fn get_stats(&self) -> ClientStats {
        if let Ok(mut stats) = self.stats.lock() {
            stats.uptime = self.created_at.elapsed();
            stats.clone()
        } else {
            ClientStats::default()
        }
    }
    
    fn get_config_info(&self) -> ClientConfigInfo {
        ClientConfigInfo {
            server_url: self.config.server_url.clone(),
            has_auth: self.config.auth_token.is_some(),
            send_format: self.config.send_format.clone(),
            timeout_secs: self.config.timeout_secs.unwrap_or(30),
            max_retries: self.config.retry_config.max_attempts,
            retry_delay_ms: self.config.retry_config.base_delay_ms,
        }
    }
}

/// Synchronous wrapper around AsyncNtfyClient for blocking operations
pub struct NtfyClient {
    inner: AsyncNtfyClient,
}

impl NtfyClient {
    /// Create a new sync client wrapping an async client
    pub fn new(async_client: AsyncNtfyClient) -> Self {
        Self {
            inner: async_client,
        }
    }
    
    /// Create a new sync client with configuration
    pub fn with_config(config: NtfyClientConfig) -> Result<Self> {
        let async_client = AsyncNtfyClient::new(config)?;
        Ok(Self::new(async_client))
    }
    
    /// Send a notification (blocking)
    pub fn send(&self, message: &NtfyMessage) -> Result<()> {
        // Use block_in_place if we're in a tokio runtime, otherwise create a new runtime
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(self.inner.send(message))
                })
            }
            Err(_) => {
                // Not in a tokio runtime, create a new one
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                rt.block_on(self.inner.send(message))
            }
        }
    }
    
    /// Send a simple notification (blocking)
    pub fn send_simple(&self, topic: &str, title: &str, message: &str, priority: u8) -> Result<()> {
        // Use block_in_place if we're in a tokio runtime, otherwise create a new runtime
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(self.inner.send_simple(topic, title, message, priority))
                })
            }
            Err(_) => {
                // Not in a tokio runtime, create a new one
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                rt.block_on(self.inner.send_simple(topic, title, message, priority))
            }
        }
    }
    
    /// Validate configuration (blocking)
    pub fn validate_config(&self) -> Result<()> {
        // Use block_in_place if we're in a tokio runtime, otherwise create a new runtime
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(self.inner.validate_config())
                })
            }
            Err(_) => {
                // Not in a tokio runtime, create a new one
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                rt.block_on(self.inner.validate_config())
            }
        }
    }
    
    /// Perform health check (blocking)
    pub fn health_check(&self) -> Result<()> {
        // Use block_in_place if we're in a tokio runtime, otherwise create a new runtime
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(self.inner.health_check())
                })
            }
            Err(_) => {
                // Not in a tokio runtime, create a new one
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                rt.block_on(self.inner.health_check())
            }
        }
    }
    
    /// Get client statistics
    pub fn get_stats(&self) -> ClientStats {
        self.inner.get_stats()
    }
    
    /// Get client configuration info
    pub fn get_config_info(&self) -> ClientConfigInfo {
        self.inner.get_config_info()
    }
}

// Convert from config types (transitional compatibility)
impl From<&crate::config::Config> for NtfyClientConfig {
    fn from(config: &crate::config::Config) -> Self {
        Self {
            server_url: config.ntfy.server_url.clone(),
            auth_token: config.ntfy.auth_token.clone(),
            timeout_secs: config.ntfy.timeout_secs,
            send_format: config.ntfy.send_format.clone(),
            retry_config: RetryConfig::default(),
            user_agent: Some("claude-ntfy/0.1.0".to_string()),
        }
    }
}

// Convert directly from NtfyConfig
impl From<&crate::config::NtfyConfig> for NtfyClientConfig {
    fn from(config: &crate::config::NtfyConfig) -> Self {
        Self {
            server_url: config.server_url.clone(),
            auth_token: config.auth_token.clone(),
            timeout_secs: config.timeout_secs,
            send_format: config.send_format.clone(),
            retry_config: RetryConfig::default(),
            user_agent: Some("claude-ntfy/0.1.0".to_string()),
        }
    }
}

// Default implementation for NtfyMessage for convenience

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;
    
    #[test]
    async fn test_client_creation() {
        let config = NtfyClientConfig::default();
        let client = AsyncNtfyClient::new(config);
        assert!(client.is_ok());
    }
    
    #[test] 
    async fn test_url_building() {
        let config = NtfyClientConfig {
            server_url: "https://ntfy.example.com".to_string(),
            ..Default::default()
        };
        let client = AsyncNtfyClient::new(config).unwrap();
        let url = client.build_url("test-topic").unwrap();
        assert_eq!(url, "https://ntfy.example.com/test-topic");
    }
    
    #[test]
    async fn test_json_body_building() {
        let config = NtfyClientConfig::default();
        let client = AsyncNtfyClient::new(config).unwrap();
        
        let message = NtfyMessage {
            topic: "test".to_string(),
            title: Some("Test Title".to_string()),
            message: "Test Message".to_string(),
            priority: Some(3),
            ..Default::default()
        };
        
        let body = client.build_json_body(&message).unwrap();
        assert_eq!(body["message"], "Test Message");
        assert_eq!(body["title"], "Test Title");
        assert_eq!(body["priority"], 3);
    }
    
    #[test]
    fn test_retry_config() {
        let config = RetryConfig::exponential(3, 100);
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 100);
        
        let delay1 = config.calculate_delay(0);
        let delay2 = config.calculate_delay(1);
        assert!(delay2 > delay1);
    }
    
    #[test]
    fn test_client_stats() {
        let mut stats = ClientStats::default();
        stats.record_success(100);
        stats.record_success(200);
        
        assert_eq!(stats.messages_sent, 2);
        assert_eq!(stats.average_latency_ms, 150);
        assert_eq!(stats.min_latency_ms, 100);
        assert_eq!(stats.max_latency_ms, 200);
        assert_eq!(stats.success_rate(), 100.0);
        
        stats.record_failure("Test error".to_string());
        assert_eq!(stats.messages_failed, 1);
        assert!(stats.success_rate() < 100.0);
    }
}