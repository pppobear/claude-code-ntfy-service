//! Error context enhancement utilities
//! 
//! This module provides traits and utilities for enhancing errors with additional
//! context information, making debugging and error handling more effective.

use std::path::PathBuf;
use super::types::AppError;

/// Error context information
/// 
/// This struct holds additional context that can be attached to errors
/// to provide more detailed information about the circumstances when the error occurred.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub component: Option<String>,
    pub file_path: Option<PathBuf>,
    pub line_number: Option<u32>,
    pub additional_info: Option<String>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            component: None,
            file_path: None,
            line_number: None,
            additional_info: None,
        }
    }
    
    /// Add component information
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = Some(component.into());
        self
    }
    
    /// Add file path information
    pub fn with_file_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = Some(path.into());
        self
    }
    
    /// Add line number information
    pub fn with_line_number(mut self, line: u32) -> Self {
        self.line_number = Some(line);
        self
    }
    
    /// Add additional information
    pub fn with_additional_info(mut self, info: impl Into<String>) -> Self {
        self.additional_info = Some(info.into());
        self
    }
    
    /// Convert to a formatted string for error messages
    pub fn to_string(&self) -> String {
        let mut parts = vec![self.operation.clone()];
        
        if let Some(ref component) = self.component {
            parts.push(format!("component: {}", component));
        }
        
        if let Some(ref path) = self.file_path {
            parts.push(format!("file: {}", path.display()));
        }
        
        if let Some(line) = self.line_number {
            parts.push(format!("line: {}", line));
        }
        
        if let Some(ref info) = self.additional_info {
            parts.push(info.clone());
        }
        
        parts.join(" | ")
    }
}

/// Extension trait for adding context to error types
/// 
/// This trait provides convenient methods for enhancing errors with contextual information,
/// similar to anyhow's context functionality but with structured data.
pub trait ErrorContextExt<T> {
    /// Add operation context to the error
    fn with_context(self, operation: impl Into<String>) -> Result<T, AppError>;
    
    /// Add operation context with a closure (lazy evaluation)
    fn with_context_lazy<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String;
    
    /// Add full error context information
    fn with_error_context(self, context: ErrorContext) -> Result<T, AppError>;
    
    /// Add component context
    fn in_component(self, component: impl Into<String>) -> Result<T, AppError>;
    
    /// Add file path context
    fn in_file(self, path: impl Into<PathBuf>) -> Result<T, AppError>;
    
    /// Add operation and file context
    fn in_file_operation(
        self, 
        path: impl Into<PathBuf>, 
        operation: impl Into<String>
    ) -> Result<T, AppError>;
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
    
    fn with_context_lazy<F>(self, f: F) -> Result<T, AppError>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let operation = f();
            AppError::Other {
                message: format!("{}: {}", operation, e),
                source: Some(Box::new(e)),
            }
        })
    }
    
    fn with_error_context(self, context: ErrorContext) -> Result<T, AppError> {
        self.map_err(|e| {
            AppError::Other {
                message: format!("{}: {}", context.to_string(), e),
                source: Some(Box::new(e)),
            }
        })
    }
    
    fn in_component(self, component: impl Into<String>) -> Result<T, AppError> {
        let component = component.into();
        self.map_err(|e| {
            AppError::Other {
                message: format!("in component '{}': {}", component, e),
                source: Some(Box::new(e)),
            }
        })
    }
    
    fn in_file(self, path: impl Into<PathBuf>) -> Result<T, AppError> {
        let path = path.into();
        self.map_err(|e| {
            AppError::Other {
                message: format!("in file '{}': {}", path.display(), e),
                source: Some(Box::new(e)),
            }
        })
    }
    
    fn in_file_operation(
        self, 
        path: impl Into<PathBuf>, 
        operation: impl Into<String>
    ) -> Result<T, AppError> {
        let path = path.into();
        let operation = operation.into();
        self.map_err(|e| {
            AppError::Other {
                message: format!("{} in file '{}': {}", operation, path.display(), e),
                source: Some(Box::new(e)),
            }
        })
    }
}

/// Macro for adding context to expressions that return Results
/// 
/// Usage:
/// ```rust
/// use crate::errors::{context, AppError};
/// 
/// fn example() -> Result<String, AppError> {
///     let content = context!(
///         std::fs::read_to_string("config.toml"),
///         "reading configuration file"
///     )?;
///     Ok(content)
/// }
/// ```
#[macro_export]
macro_rules! context {
    ($expr:expr, $context:expr) => {
        $crate::errors::ErrorContextExt::with_context($expr, $context)
    };
}

/// Create an ErrorContext with file and line information
#[macro_export]
macro_rules! error_context {
    ($operation:expr) => {
        $crate::errors::ErrorContext::new($operation)
            .with_file_path(file!())
            .with_line_number(line!())
    };
    ($operation:expr, $component:expr) => {
        $crate::errors::ErrorContext::new($operation)
            .with_component($component)
            .with_file_path(file!())
            .with_line_number(line!())
    };
}