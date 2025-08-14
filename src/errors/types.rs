//! Comprehensive error types for the claude-ntfy service
//! 
//! This module defines all error types that can occur throughout the application,
//! providing structured error handling with proper context and source chains.

use thiserror::Error;
use std::path::PathBuf;

/// Main application error type
/// 
/// This enum covers all possible error conditions in the claude-ntfy service,
/// organized by functional domain for better error handling and debugging.
#[derive(Error, Debug)]
pub enum AppError {
    // Configuration errors
    #[error("Configuration error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Configuration file not found: {path}")]
    ConfigNotFound {
        path: PathBuf,
    },
    
    #[error("Invalid configuration value for '{key}': {value}")]
    InvalidConfigValue {
        key: String,
        value: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // Hook processing errors
    #[error("Hook processing failed for '{hook_name}': {message}")]
    HookProcessing {
        hook_name: String,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Hook validation failed for '{hook_name}': {reason}")]
    HookValidation {
        hook_name: String,
        reason: String,
    },
    
    #[error("Hook data size ({size} bytes) exceeds limit ({limit} bytes) for '{hook_name}'")]
    HookDataSizeLimit {
        hook_name: String,
        size: usize,
        limit: usize,
    },
    
    #[error("Hook '{hook_name}' is not allowed by configuration")]
    HookNotAllowed {
        hook_name: String,
    },
    
    #[error("Validation error: {0}")]
    ValidationError(String),

    // Notification client errors
    #[error("Notification client error: {message}")]
    NotificationClient {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Failed to send notification to '{topic}': {reason}")]
    NotificationSendFailed {
        topic: String,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Invalid ntfy server URL: {url}")]
    InvalidNtfyUrl {
        url: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Notification retry limit exceeded: {attempts} attempts for '{topic}'")]
    NotificationRetryExhausted {
        topic: String,
        attempts: u32,
    },
    
    #[error("Authentication failed for ntfy server")]
    NtfyAuthenticationFailed {
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // Daemon errors
    #[error("Daemon error: {message}")]
    Daemon {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Failed to start daemon: {reason}")]
    DaemonStartFailed {
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Daemon communication failed: {operation}")]
    DaemonCommunication {
        operation: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("IPC error: {message}")]
    Ipc {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Socket error at '{path}': {operation}")]
    Socket {
        path: PathBuf,
        operation: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // Template processing errors
    #[error("Template error: {message}")]
    Template {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Template not found: {name}")]
    TemplateNotFound {
        name: String,
    },
    
    #[error("Template syntax error in '{template_name}': {reason}")]
    TemplateSyntax {
        template_name: String,
        reason: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Template rendering failed for '{template_name}': {context}")]
    TemplateRendering {
        template_name: String,
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // I/O errors
    #[error("File I/O error for '{path}': {operation}")]
    Io {
        path: PathBuf,
        operation: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("File not found: {path}")]
    FileNotFound {
        path: PathBuf,
    },
    
    #[error("Permission denied for '{path}': {operation}")]
    PermissionDenied {
        path: PathBuf,
        operation: String,
    },

    // Serialization errors
    #[error("JSON serialization error: {context}")]
    JsonSerialization {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("JSON deserialization error: {context}")]
    JsonDeserialization {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("TOML parsing error: {context}")]
    TomlParsing {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // CLI and command errors
    #[error("CLI error: {message}")]
    Cli {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Invalid command argument '{argument}': {reason}")]
    InvalidArgument {
        argument: String,
        reason: String,
    },
    
    #[error("Missing required argument: {argument}")]
    MissingArgument {
        argument: String,
    },

    // Network and HTTP errors
    #[error("HTTP request failed: {method} {url}")]
    HttpRequest {
        method: String,
        url: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Network timeout after {timeout_secs} seconds")]
    NetworkTimeout {
        timeout_secs: u64,
    },
    
    #[error("HTTP {status_code}: {reason}")]
    HttpStatus {
        status_code: u16,
        reason: String,
    },

    // System and process errors
    #[error("System error: {message}")]
    System {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Process execution failed: {command}")]
    ProcessExecution {
        command: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("Environment variable not found: {var_name}")]
    EnvVarNotFound {
        var_name: String,
    },

    // Validation and parsing errors
    #[error("Validation error: {field} - {reason}")]
    Validation {
        field: String,
        reason: String,
    },
    
    #[error("Parse error for '{input}': {expected}")]
    Parse {
        input: String,
        expected: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    // Generic/catch-all errors
    #[error("Internal error: {message}")]
    Internal {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
    
    #[error("{message}")]
    Other {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// Convenience type alias for Results using AppError
pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// Create a new Config error with context
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new Config error with source
    pub fn config_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Create a new HookProcessing error
    pub fn hook_processing(hook_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::HookProcessing {
            hook_name: hook_name.into(),
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new HookProcessing error with source
    pub fn hook_processing_with_source(
        hook_name: impl Into<String>,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::HookProcessing {
            hook_name: hook_name.into(),
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Create a new NotificationClient error
    pub fn notification_client(message: impl Into<String>) -> Self {
        Self::NotificationClient {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new NotificationClient error with source
    pub fn notification_client_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::NotificationClient {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Create a new I/O error
    pub fn io(path: impl Into<PathBuf>, operation: impl Into<String>) -> Self {
        Self::Io {
            path: path.into(),
            operation: operation.into(),
            source: None,
        }
    }
    
    /// Create a new I/O error with source
    pub fn io_with_source(
        path: impl Into<PathBuf>,
        operation: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Io {
            path: path.into(),
            operation: operation.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Create a new Template error
    pub fn template(message: impl Into<String>) -> Self {
        Self::Template {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new Template error with source
    pub fn template_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Template {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Create a new Internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            source: None,
        }
    }
    
    /// Create a new Internal error with source
    pub fn internal_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Internal {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
    
    /// Check if this error is retryable (useful for notification sending)
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::NetworkTimeout { .. } => true,
            Self::HttpRequest { .. } => true,
            Self::HttpStatus { status_code, .. } => {
                // Retry on 5xx errors and some 4xx errors
                *status_code >= 500 || *status_code == 408 || *status_code == 429
            },
            Self::NotificationSendFailed { .. } => true,
            Self::DaemonCommunication { .. } => true,
            Self::Ipc { .. } => true,
            Self::Socket { .. } => true,
            _ => false,
        }
    }
    
    /// Get the error category for metrics and logging
    pub fn category(&self) -> &'static str {
        match self {
            Self::Config { .. } | Self::ConfigNotFound { .. } | Self::InvalidConfigValue { .. } => "config",
            Self::HookProcessing { .. } | Self::HookValidation { .. } | Self::HookDataSizeLimit { .. } | Self::HookNotAllowed { .. } | Self::ValidationError(_) => "hook",
            Self::NotificationClient { .. } | Self::NotificationSendFailed { .. } | Self::InvalidNtfyUrl { .. } | Self::NotificationRetryExhausted { .. } | Self::NtfyAuthenticationFailed { .. } => "notification",
            Self::Daemon { .. } | Self::DaemonStartFailed { .. } | Self::DaemonCommunication { .. } | Self::Ipc { .. } | Self::Socket { .. } => "daemon",
            Self::Template { .. } | Self::TemplateNotFound { .. } | Self::TemplateSyntax { .. } | Self::TemplateRendering { .. } => "template",
            Self::Io { .. } | Self::FileNotFound { .. } | Self::PermissionDenied { .. } => "io",
            Self::JsonSerialization { .. } | Self::JsonDeserialization { .. } | Self::TomlParsing { .. } => "serialization",
            Self::Cli { .. } | Self::InvalidArgument { .. } | Self::MissingArgument { .. } => "cli",
            Self::HttpRequest { .. } | Self::NetworkTimeout { .. } | Self::HttpStatus { .. } => "network",
            Self::System { .. } | Self::ProcessExecution { .. } | Self::EnvVarNotFound { .. } => "system",
            Self::Validation { .. } | Self::Parse { .. } => "validation",
            Self::Internal { .. } | Self::Other { .. } => "internal",
        }
    }
}

// Implement conversions from common standard library and third-party error types
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        let operation = match err.kind() {
            std::io::ErrorKind::NotFound => "file not found",
            std::io::ErrorKind::PermissionDenied => "permission denied",
            std::io::ErrorKind::ConnectionRefused => "connection refused",
            std::io::ErrorKind::ConnectionAborted => "connection aborted",
            std::io::ErrorKind::TimedOut => "timeout",
            _ => "I/O operation",
        }.to_string();
        
        Self::Io {
            path: PathBuf::from("unknown"),
            operation,
            source: Some(Box::new(err)),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        if err.is_syntax() {
            Self::JsonDeserialization {
                context: format!("JSON syntax error at line {} column {}", 
                    err.line(), err.column()),
                source: Some(Box::new(err)),
            }
        } else if err.is_data() {
            Self::JsonDeserialization {
                context: "JSON data error".to_string(),
                source: Some(Box::new(err)),
            }
        } else if err.is_eof() {
            Self::JsonDeserialization {
                context: "Unexpected end of JSON input".to_string(),
                source: Some(Box::new(err)),
            }
        } else {
            Self::JsonSerialization {
                context: "JSON serialization error".to_string(),
                source: Some(Box::new(err)),
            }
        }
    }
}

impl From<toml::de::Error> for AppError {
    fn from(err: toml::de::Error) -> Self {
        Self::TomlParsing {
            context: err.to_string(),
            source: Some(Box::new(err)),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::NetworkTimeout {
                timeout_secs: 0, // We don't have access to the actual timeout value
            }
        } else if err.is_connect() {
            Self::HttpRequest {
                method: "UNKNOWN".to_string(),
                url: err.url().map(|u| u.to_string()).unwrap_or_else(|| "unknown".to_string()),
                source: Some(Box::new(err)),
            }
        } else if let Some(status) = err.status() {
            Self::HttpStatus {
                status_code: status.as_u16(),
                reason: err.to_string(),
            }
        } else {
            Self::HttpRequest {
                method: "UNKNOWN".to_string(),
                url: err.url().map(|u| u.to_string()).unwrap_or_else(|| "unknown".to_string()),
                source: Some(Box::new(err)),
            }
        }
    }
}

impl From<url::ParseError> for AppError {
    fn from(err: url::ParseError) -> Self {
        Self::Parse {
            input: "URL".to_string(),
            expected: "valid URL format".to_string(),
            source: Some(Box::new(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let err = AppError::config("test config error");
        assert_eq!(err.to_string(), "Configuration error: test config error");
    }
    
    #[test]
    fn test_error_category() {
        let config_err = AppError::config("test");
        assert_eq!(config_err.category(), "config");
        
        let hook_err = AppError::hook_processing("test-hook", "test error");
        assert_eq!(hook_err.category(), "hook");
    }
    
    #[test]
    fn test_retryable_errors() {
        let timeout_err = AppError::NetworkTimeout { timeout_secs: 30 };
        assert!(timeout_err.is_retryable());
        
        let config_err = AppError::config("test");
        assert!(!config_err.is_retryable());
    }
    
    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        
        match app_err {
            AppError::Io { operation, .. } => {
                assert_eq!(operation, "file not found");
            },
            _ => panic!("Wrong error type"),
        }
    }
}