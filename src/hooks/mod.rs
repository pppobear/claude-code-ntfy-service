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
pub use processor::DefaultHookProcessor;


/// Create a default hook processor with standard configuration
pub fn create_default_processor() -> DefaultHookProcessor {
    let enhancer = enhancer::DefaultHookDataEnhancer::new();
    let validator = validator::DefaultHookValidator::new();
    DefaultHookProcessor::new(enhancer, validator)
}