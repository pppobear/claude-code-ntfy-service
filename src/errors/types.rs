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




    // I/O errors
    #[error("File I/O error for '{path}': {operation}")]
    Io {
        path: PathBuf,
        operation: String,
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


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let err = AppError::config("test config error");
        assert_eq!(err.to_string(), "Configuration error: test config error");
    }
    
    
    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        
        match app_err {
            AppError::Io { operation, .. } => {
                assert_eq!(operation, "file not found");
            },
            _ => assert!(false, "Expected AppError::Io, got {:?}", app_err),
        }
    }
}