# Clients Module - Unified Notification Client Implementation

## Overview

The `clients` module provides a unified, async-first approach to notification clients with TypeScript-like async patterns. This implementation eliminates the ~200 lines of code duplication between sync and async implementations in the original `ntfy.rs` file.

## Architecture

```
src/clients/
├── mod.rs          # Module exports and convenience functions  
├── traits.rs       # NotificationClient trait and supporting types
├── ntfy.rs         # Unified AsyncNtfyClient + NtfyClient wrapper
├── factory.rs      # ClientFactory pattern for dependency injection
└── README.md       # This documentation
```

## Key Features

### ✅ Async-First Design
- **AsyncNtfyClient**: Primary async implementation with advanced features
- **NtfyClient**: Sync wrapper using `tokio::runtime::Handle::current().block_on()`
- Single codebase eliminates sync/async duplication

### ✅ TypeScript-like Patterns
- Promise-like error handling with `Result<T>`
- Unified async interfaces with `.await` syntax
- Strong typing with comprehensive trait boundaries
- Factory pattern for dependency injection

### ✅ Advanced Features
- **Retry Logic**: Configurable exponential backoff with jitter
- **Statistics**: Performance tracking and health monitoring  
- **HTTP Optimization**: Connection pooling, keepalive, timeouts
- **Error Context**: Detailed error messages with `anyhow::Context`

### ✅ Compatibility Layer
- Drop-in replacement for existing sync/async clients
- Configuration conversion from existing `Config` types
- Legacy compatibility functions for smooth migration

## Usage Examples

### Basic Usage

```rust
use claude_ntfy::clients::{AsyncNtfyClient, NtfyClientConfig};

// Create client with configuration
let config = NtfyClientConfig {
    server_url: "https://ntfy.sh".to_string(),
    auth_token: Some("your-token".to_string()),
    send_format: "json".to_string(),
    ..Default::default()
};

let client = AsyncNtfyClient::new(config)?;

// Send notification
client.send_simple("topic", "Title", "Message", 3).await?;

// Create sync wrapper
let sync_client = client.blocking();
sync_client.send_simple("topic", "Title", "Message", 3)?;
```

### Factory Pattern

```rust
use claude_ntfy::clients::{ClientFactory, DefaultClientFactory};
use claude_ntfy::config::Config;

let factory = DefaultClientFactory::new();
let client = factory.create_from_app_config(&config)?;
```

### Convenience Functions

```rust
use claude_ntfy::clients::convenience;

// Quick client creation
let client = convenience::create_default_client()?;
let fast_client = convenience::create_high_performance_client("https://ntfy.sh", None)?;
let reliable_client = convenience::create_reliable_client("https://ntfy.sh", None)?;
```

### Statistics and Monitoring

```rust
// Get performance statistics
let stats = client.get_stats();
println!("Success rate: {:.1}%", stats.success_rate());
println!("Average latency: {}ms", stats.average_latency_ms);

// Health check
client.health_check().await?;
client.validate_config().await?;
```

## Integration Points

### For Daemon Implementation

```rust
// In daemon/server.rs
use crate::clients::{create_client_from_config, NotificationClient};

pub struct NotificationDaemon {
    client: Box<dyn NotificationClient>,
    // ... other fields
}

impl NotificationDaemon {
    pub async fn new(config: &Config) -> Result<Self> {
        let client = create_client_from_config(config)?;
        Ok(Self { client })
    }
    
    pub async fn process_notification(&self, message: &NtfyMessage) -> Result<()> {
        self.client.send(message).await
    }
}
```

### For CLI Handlers

```rust
// In cli/handlers.rs
use crate::clients::{create_sync_client_from_ntfy_config, NtfyClient};

impl CommandHandler {
    pub fn handle_send_notification(&self, topic: &str, message: &str) -> Result<()> {
        let client = create_sync_client_from_ntfy_config(&self.config.ntfy)?;
        client.send_simple(topic, "CLI Notification", message, 3)?;
        Ok(())
    }
}
```

## Migration from Old ntfy.rs

### Before (Duplicated Code)
```rust
// Old sync client - ~250 lines
pub struct NtfyClient { /* ... */ }
impl NtfyClient {
    pub fn send(&self, message: &NtfyMessage) -> Result<()> {
        // HTTP client setup, headers, body building, sending...
    }
}

// Old async client - ~250 lines (mostly duplicated)
pub struct AsyncNtfyClient { /* ... */ }
impl AsyncNtfyClient {
    pub async fn send(&self, message: &NtfyMessage) -> Result<()> {
        // Same HTTP client setup, headers, body building, sending...
    }
}
```

### After (Unified Implementation)
```rust
// New unified implementation - single codebase
pub struct AsyncNtfyClient { /* ... */ }
impl AsyncNtfyClient {
    pub async fn send(&self, message: &NtfyMessage) -> Result<()> {
        // Single implementation with retry, stats, optimization
    }
    
    pub fn blocking(self) -> NtfyClient {
        NtfyClient::new(self) // Sync wrapper
    }
}

pub struct NtfyClient {
    inner: AsyncNtfyClient,
    runtime: tokio::runtime::Handle,
}
```

## Configuration

### Client Configuration
```rust
pub struct NtfyClientConfig {
    pub server_url: String,
    pub auth_token: Option<String>, 
    pub timeout_secs: Option<u64>,
    pub send_format: String,          // "json" or "text"
    pub retry_config: RetryConfig,
    pub user_agent: Option<String>,
}
```

### Retry Configuration
```rust
pub struct RetryConfig {
    pub max_attempts: u32,           // Default: 3
    pub base_delay_ms: u64,          // Default: 100ms
    pub max_delay_ms: u64,           // Default: 5000ms
    pub backoff_multiplier: f64,     // Default: 2.0 (exponential)
    pub jitter_factor: f64,          // Default: 0.1 (10% jitter)
}
```

## Error Handling

The clients module provides comprehensive error handling with context:

```rust
use anyhow::{Context, Result};

// Detailed error context
client.send(message).await
    .context("Failed to send notification to ntfy server")?;

// Health check with specific error info  
client.health_check().await
    .context("Health check failed - server may be unreachable")?;
```

## Performance Optimizations

### HTTP Client Configuration
- Connection pooling with idle timeout (90s)
- TCP keepalive (60s) 
- Pool max idle per host (10 connections)
- Configurable request timeouts

### Retry Logic
- Exponential backoff with jitter to prevent thundering herd
- Smart retry attempts based on send format
- Configurable delay bounds

### Statistics Tracking
- Latency percentiles (min/avg/max)
- Success/failure rates
- Retry attempt tracking
- Zero-cost when not accessed

## Dependencies Added

```toml
[dependencies]
async-trait = "0.1"    # For clean async trait definitions
rand = "0.8"           # For retry jitter calculation
```

## Testing

The module includes comprehensive tests:
- Unit tests for client creation and configuration
- Integration tests for config conversion
- Mock factory for testing scenarios
- Statistics calculation verification
- Retry logic validation

## Summary

This unified clients module provides:

1. **200+ lines of eliminated duplication** between sync/async implementations
2. **TypeScript-like async patterns** for familiar development experience  
3. **Production-ready features** including retry logic, statistics, and health monitoring
4. **Clean integration points** for both daemon and CLI usage
5. **Backward compatibility** with existing configuration types
6. **Comprehensive error handling** with detailed context
7. **Performance optimizations** for high-throughput scenarios

The module is ready for integration into both the daemon and CLI handlers, providing a modern, maintainable foundation for notification services.