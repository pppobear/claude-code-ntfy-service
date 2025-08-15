//! Error context enhancement utilities
//! 
//! This module provides traits and utilities for enhancing errors with additional
//! context information, making debugging and error handling more effective.

use super::types::AppError;

/// Extension trait for adding context to error types
/// 
/// This trait provides convenient methods for enhancing errors with contextual information,
/// similar to anyhow's context functionality but with structured data.
pub trait ErrorContextExt<T> {
    /// Add operation context to the error
    fn with_context(self, operation: impl Into<String>) -> Result<T, AppError>;
}

// Generic implementation for all error types that implement std::error::Error
impl<T, E> ErrorContextExt<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_context(self, operation: impl Into<String>) -> Result<T, AppError> {
        self.map_err(|e| {
            let operation = operation.into();
            AppError::Other {
                message: format!("{}: {}", operation, e),
                source: Some(Box::new(e)),
            }
        })
    }
}

