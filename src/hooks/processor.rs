//! Hook processing pipeline
//! 
//! This module contains the main hook processing logic, including the
//! HookProcessor trait and DefaultHookProcessor implementation.

use serde_json::Value;
use std::sync::Arc;

use crate::errors::{AppError, AppResult, ErrorContextExt};
use super::types::{ProcessedHook, HookMetadata, HookConfig, SystemInfo, ClaudeEnvironment, GitInfo, UserInfo};
use super::enhancer::HookDataEnhancer;
use super::validator::HookValidator;

/// Trait for hook processors
/// 
/// This trait defines the interface for processing hooks. Implementations
/// should handle the full pipeline from raw hook data to ProcessedHook.
pub trait HookProcessor: Send + Sync {
    /// Process a hook with the given name and data
    /// 
    /// # Arguments
    /// * `hook_name` - The name of the hook being processed
    /// * `data` - The hook data to process
    /// 
    /// # Returns
    /// A ProcessedHook with enhanced data and metadata, or an error
    fn process(&self, hook_name: &str, data: Value) -> AppResult<ProcessedHook>;
    
}

/// Default implementation of HookProcessor
/// 
/// This processor handles the standard pipeline:
/// 1. Validation of input data
/// 2. Data enhancement (e.g., inferring success fields)
/// 3. Metadata collection
/// 4. Final validation of processed data
pub struct DefaultHookProcessor {
    enhancer: Arc<dyn HookDataEnhancer>,
    validator: Arc<dyn HookValidator>,
    config: HookConfig,
}

impl DefaultHookProcessor {
    /// Create a new DefaultHookProcessor with the given enhancer and validator
    pub fn new(
        enhancer: impl HookDataEnhancer + 'static,
        validator: impl HookValidator + 'static,
    ) -> Self {
        Self {
            enhancer: Arc::new(enhancer),
            validator: Arc::new(validator),
            config: HookConfig::default(),
        }
    }
    
    /// Create a new DefaultHookProcessor with custom configuration
    #[allow(dead_code)]
    pub fn with_config(
        enhancer: impl HookDataEnhancer + 'static,
        validator: impl HookValidator + 'static,
        config: HookConfig,
    ) -> Self {
        Self {
            enhancer: Arc::new(enhancer),
            validator: Arc::new(validator),
            config,
        }
    }
    
    /// Check if the hook should be processed based on configuration
    fn should_process_hook(&self, hook_name: &str) -> bool {
        // Check if hook is in ignored list
        if self.config.ignored_hooks.contains(&hook_name.to_string()) {
            return false;
        }
        
        // If allowed_hooks is specified, check if hook is in the list
        if !self.config.allowed_hooks.is_empty() {
            return self.config.allowed_hooks.contains(&hook_name.to_string());
        }
        
        true
    }
    
    /// Collect metadata for the hook
    fn collect_metadata(&self) -> AppResult<HookMetadata> {
        let system_info = if self.config.collect_system_info {
            SystemInfo::current()
        } else {
            SystemInfo {
                os: "unknown".to_string(),
                arch: "unknown".to_string(),
                hostname: None,
                cwd: None,
                pid: 0,
            }
        };
        
        let git_info = if self.config.collect_git_info {
            self.collect_git_info().ok()
        } else {
            None
        };
        
        let user_info = self.collect_user_info().ok();
        
        let environment = std::env::vars().collect();
        let claude_env = ClaudeEnvironment::from_env();
        
        Ok(HookMetadata {
            git_info,
            user_info,
            system_info,
            environment,
            claude_env,
        })
    }
    
    /// Collect git repository information
    fn collect_git_info(&self) -> AppResult<GitInfo> {
        // This will be implemented with actual git commands
        // For now, return a placeholder
        Ok(GitInfo {
            branch: None,
            commit: None,
            repo_root: None,
            has_changes: false,
            remote_url: None,
        })
    }
    
    /// Collect user information
    fn collect_user_info(&self) -> AppResult<UserInfo> {
        // This will be implemented with actual user detection
        // For now, return a placeholder
        Ok(UserInfo {
            username: std::env::var("USER").ok().or_else(|| std::env::var("USERNAME").ok()),
            full_name: None,
            email: None,
        })
    }
}

impl HookProcessor for DefaultHookProcessor {
    fn process(&self, hook_name: &str, data: Value) -> AppResult<ProcessedHook> {
        // Check if we should process this hook
        if !self.should_process_hook(hook_name) {
            return Err(AppError::HookNotAllowed {
                hook_name: hook_name.to_string(),
            });
        }
        
        // Check data size limits
        if let Some(max_size) = self.config.max_data_size {
            let data_size = serde_json::to_string(&data)
                .with_context("Failed to serialize hook data for size check")?
                .len();
            if data_size > max_size {
                return Err(AppError::HookDataSizeLimit {
                    hook_name: hook_name.to_string(),
                    size: data_size,
                    limit: max_size,
                });
            }
        }
        
        // Initial validation
        if self.config.enable_validation {
            self.validator.validate_input(hook_name, &data)
                .with_context("Initial hook data validation failed")?;
        }
        
        // Enhance the hook data
        let enhanced_data = if self.config.enable_enhancement {
            self.enhancer.enhance(hook_name, data.clone())
                .with_context("Hook data enhancement failed")?
        } else {
            data.clone()
        };
        
        // Collect metadata
        let metadata = self.collect_metadata()
            .with_context("Failed to collect hook metadata")?;
        
        // Create the processed hook
        let processed_hook = ProcessedHook::new(
            hook_name.to_string(),
            data,
            enhanced_data,
            metadata,
        );
        
        // Final validation
        if self.config.enable_validation {
            self.validator.validate_processed(&processed_hook)
                .with_context("Final processed hook validation failed")?;
        }
        
        Ok(processed_hook)
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    // Mock implementations for testing
    struct MockEnhancer;
    impl HookDataEnhancer for MockEnhancer {
        fn enhance(&self, _hook_name: &str, data: Value) -> AppResult<Value> {
            Ok(data)
        }
    }
    
    struct MockValidator;
    impl HookValidator for MockValidator {
        fn validate_input(&self, _hook_name: &str, _data: &Value) -> AppResult<()> {
            Ok(())
        }
        
        fn validate_processed(&self, _hook: &ProcessedHook) -> AppResult<()> {
            Ok(())
        }
    }
    
    #[test]
    fn test_default_processor_creation() {
        let _processor = DefaultHookProcessor::new(MockEnhancer, MockValidator);
        // Just verify processor can be created successfully
    }
    
    #[test]
    fn test_hook_processing() {
        let processor = DefaultHookProcessor::new(MockEnhancer, MockValidator);
        let hook_data = json!({"test": "data"});
        
        let result = processor.process("PostToolUse", hook_data);
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.hook_name, "PostToolUse");
        assert_eq!(processed.original_data, json!({"test": "data"}));
    }

    #[test]
    fn test_hook_processing_basic() {
        let processor = DefaultHookProcessor::new(MockEnhancer, MockValidator);
        let hook_data = json!({"tool_name": "Read", "file_path": "/test/file.txt"});
        
        let result = processor.process("PostToolUse", hook_data.clone());
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        assert_eq!(processed.hook_name, "PostToolUse");
        assert_eq!(processed.original_data, hook_data);
        assert_eq!(processed.enhanced_data, hook_data); // MockEnhancer returns same data
    }

    #[test]
    fn test_metadata_collection() {
        let processor = DefaultHookProcessor::new(MockEnhancer, MockValidator);
        let result = processor.process("PostToolUse", json!({"test": "data"}));
        assert!(result.is_ok());
        
        let processed = result.unwrap();
        // Metadata is always present in ProcessedHook
        assert!(!processed.metadata.system_info.os.is_empty());
        assert!(processed.metadata.system_info.pid > 0);
    }

    #[test]
    fn test_different_hook_types() {
        let processor = DefaultHookProcessor::new(MockEnhancer, MockValidator);
        
        let hook_types = vec![
            "PreToolUse",
            "PostToolUse", 
            "UserPromptSubmit",
            "SessionStart",
            "Stop"
        ];
        
        for hook_type in hook_types {
            let result = processor.process(hook_type, json!({"test": "data"}));
            assert!(result.is_ok(), "Failed to process hook type: {}", hook_type);
            
            let processed = result.unwrap();
            assert_eq!(processed.hook_name, hook_type);
        }
    }

    // Mock enhancer that fails
    struct FailingEnhancer;
    impl HookDataEnhancer for FailingEnhancer {
        fn enhance(&self, _hook_name: &str, _data: Value) -> AppResult<Value> {
            Err(AppError::HookProcessing {
                hook_name: "test".to_string(),
                message: "Enhancement failed".to_string(),
                source: None,
            })
        }
    }

    #[test]
    fn test_enhancement_failure() {
        let processor = DefaultHookProcessor::new(FailingEnhancer, MockValidator);
        let result = processor.process("PostToolUse", json!({"test": "data"}));
        assert!(result.is_err());
    }

    // Mock validator that fails
    struct FailingValidator;
    impl HookValidator for FailingValidator {
        fn validate_input(&self, _hook_name: &str, _data: &Value) -> AppResult<()> {
            Err(AppError::ValidationError("Validation failed".to_string()))
        }
        
        fn validate_processed(&self, _hook: &ProcessedHook) -> AppResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_validation_failure() {
        let processor = DefaultHookProcessor::new(MockEnhancer, FailingValidator);
        let result = processor.process("PostToolUse", json!({"test": "data"}));
        assert!(result.is_err());
    }

}