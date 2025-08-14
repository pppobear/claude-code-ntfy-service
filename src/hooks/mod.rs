//! Hooks processing module for claude-code-ntfy-service
//! 
//! This module provides centralized hook processing functionality, extracted from
//! the monolithic handlers.rs implementation. It supports multiple hook types
//! with data enhancement, validation, and metadata collection.

pub mod types;
pub mod processor;
pub mod enhancer;
pub mod validator;

// Re-export main types and traits for convenient usage
pub use types::ProcessedHook;
pub use processor::{HookProcessor, DefaultHookProcessor};

use crate::errors::AppResult;
use serde_json::Value;

/// Main entry point for hook processing
/// 
/// This function provides a simplified interface for processing hooks
/// with default configuration and processors.
pub fn process_hook(hook_name: &str, hook_data: Value) -> AppResult<ProcessedHook> {
    let enhancer = enhancer::DefaultHookDataEnhancer::new();
    let validator = validator::DefaultHookValidator::new();
    let processor = processor::DefaultHookProcessor::new(enhancer, validator);
    
    processor.process(hook_name, hook_data)
}

/// Create a default hook processor with standard configuration
pub fn create_default_processor() -> DefaultHookProcessor {
    let enhancer = enhancer::DefaultHookDataEnhancer::new();
    let validator = validator::DefaultHookValidator::new();
    DefaultHookProcessor::new(enhancer, validator)
}