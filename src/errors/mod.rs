//! Centralized error handling module
//! 
//! This module provides a unified error handling approach for the claude-ntfy service,
//! replacing the ad-hoc use of anyhow::Error with structured, typed errors.

pub mod types;
pub mod context;

pub use types::{AppError, AppResult};
pub use context::ErrorContextExt;


/// Convert from anyhow::Error to AppError for migration compatibility
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Other {
            message: err.to_string(),
            source: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_anyhow_conversion() {
        let anyhow_err = anyhow::anyhow!("test error");
        let app_err: AppError = anyhow_err.into();
        
        match app_err {
            AppError::Other { message, .. } => {
                assert_eq!(message, "test error");
            },
            _ => assert!(false, "Expected AppError::Other, got {:?}", app_err),
        }
    }
}