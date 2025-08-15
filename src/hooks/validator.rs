//! Hook validation logic
//! 
//! This module contains validation logic for hooks, including input validation,
//! security checks, and processed hook validation.

use serde_json::Value;
use std::collections::HashSet;
use crate::errors::{AppError, AppResult, ErrorContextExt};
use super::types::ProcessedHook;

/// Trait for hook validators
/// 
/// Validators ensure hook data meets security and format requirements
/// before and after processing.
pub trait HookValidator: Send + Sync {
    /// Validate input hook data before processing
    /// 
    /// # Arguments
    /// * `hook_name` - The name of the hook being validated
    /// * `data` - The hook data to validate
    /// 
    /// # Returns
    /// Ok(()) if validation passes, Err if validation fails
    fn validate_input(&self, hook_name: &str, data: &Value) -> AppResult<()>;
    
    /// Validate processed hook after enhancement
    /// 
    /// # Arguments
    /// * `hook` - The processed hook to validate
    /// 
    /// # Returns
    /// Ok(()) if validation passes, Err if validation fails
    fn validate_processed(&self, hook: &ProcessedHook) -> AppResult<()>;
}

/// Default implementation of HookValidator
/// 
/// This validator implements security checks, format validation,
/// and data integrity verification.
pub struct DefaultHookValidator {
    /// Maximum allowed depth for nested JSON objects
    max_depth: usize,
    
    /// Maximum allowed string length
    max_string_length: usize,
    
    /// Forbidden field names (security)
    forbidden_fields: HashSet<String>,
    
    /// Required fields for specific hook types
    required_fields: std::collections::HashMap<String, Vec<String>>,
}

impl DefaultHookValidator {
    /// Create a new DefaultHookValidator with default settings
    pub fn new() -> Self {
        let mut forbidden_fields = HashSet::new();
        forbidden_fields.insert("password".to_string());
        forbidden_fields.insert("secret".to_string());
        forbidden_fields.insert("token".to_string());
        forbidden_fields.insert("api_key".to_string());
        forbidden_fields.insert("private_key".to_string());
        
        let mut required_fields = std::collections::HashMap::new();
        required_fields.insert("PostToolUse".to_string(), vec![]);
        required_fields.insert("PreTask".to_string(), vec!["task_id".to_string()]);
        required_fields.insert("PostTask".to_string(), vec!["task_id".to_string()]);
        
        Self {
            max_depth: 10,
            max_string_length: 1_000_000, // Increased to 1MB for Claude Code hooks
            forbidden_fields,
            required_fields,
        }
    }
    
    /// Create a new DefaultHookValidator with custom settings
    #[allow(dead_code)]
    pub fn with_config(
        max_depth: usize,
        max_string_length: usize,
        forbidden_fields: HashSet<String>,
    ) -> Self {
        let mut required_fields = std::collections::HashMap::new();
        required_fields.insert("PostToolUse".to_string(), vec![]);
        required_fields.insert("PreTask".to_string(), vec!["task_id".to_string()]);
        required_fields.insert("PostTask".to_string(), vec!["task_id".to_string()]);
        
        Self {
            max_depth,
            max_string_length,
            forbidden_fields,
            required_fields,
        }
    }
    
    /// Validate JSON structure and depth
    fn validate_json_structure(&self, data: &Value, current_depth: usize) -> AppResult<()> {
        if current_depth > self.max_depth {
            return Err(AppError::ValidationError(format!(
                "JSON structure exceeds maximum depth of {}",
                self.max_depth
            )));
        }
        
        match data {
            Value::Object(obj) => {
                for (key, value) in obj {
                    // Check for forbidden field names
                    if self.forbidden_fields.contains(&key.to_lowercase()) {
                        return Err(AppError::ValidationError(format!(
                            "Field '{}' is forbidden for security reasons",
                            key
                        )));
                    }
                    
                    // Recursively validate nested objects
                    self.validate_json_structure(value, current_depth + 1)?;
                }
            },
            Value::Array(arr) => {
                for item in arr {
                    self.validate_json_structure(item, current_depth + 1)?;
                }
            },
            Value::String(s) => {
                if s.len() > self.max_string_length {
                    return Err(AppError::ValidationError(format!(
                        "String length ({}) exceeds maximum allowed length ({})",
                        s.len(),
                        self.max_string_length
                    )));
                }
                
                // Check for potential security issues in strings
                self.validate_string_content(s)?;
            },
            _ => {}, // Other types are OK
        }
        
        Ok(())
    }
    
    /// Validate string content for security issues
    fn validate_string_content(&self, content: &str) -> AppResult<()> {
        // Check for potential SQL injection patterns
        let sql_patterns = [
            "'; DROP TABLE",
            "'; DELETE FROM",
            "'; INSERT INTO",
            "'; UPDATE ",
            "UNION SELECT",
        ];
        
        let content_upper = content.to_uppercase();
        for pattern in &sql_patterns {
            if content_upper.contains(pattern) {
                return Err(AppError::ValidationError(format!(
                    "String content contains potential SQL injection pattern: {}",
                    pattern
                )));
            }
        }
        
        // Check for script injection patterns
        let script_patterns = [
            "<script",
            "javascript:",
            "onload=",
            "onerror=",
        ];
        
        let content_lower = content.to_lowercase();
        for pattern in &script_patterns {
            if content_lower.contains(pattern) {
                return Err(AppError::ValidationError(format!(
                    "String content contains potential script injection pattern: {}",
                    pattern
                )));
            }
        }
        
        Ok(())
    }
    
    /// Validate required fields for specific hook types
    fn validate_required_fields(&self, hook_name: &str, data: &Value) -> AppResult<()> {
        if let Some(required) = self.required_fields.get(hook_name) {
            if let Some(obj) = data.as_object() {
                for field in required {
                    if !obj.contains_key(field) {
                        return Err(AppError::ValidationError(format!(
                            "Required field '{}' is missing for hook '{}'",
                            field,
                            hook_name
                        )));
                    }
                }
            } else if !required.is_empty() {
                return Err(AppError::ValidationError(format!(
                    "Hook '{}' requires object data with fields: {:?}",
                    hook_name,
                    required
                )));
            }
        }
        
        Ok(())
    }
    
    /// Validate hook name format
    fn validate_hook_name(&self, hook_name: &str) -> AppResult<()> {
        if hook_name.is_empty() {
            return Err(AppError::ValidationError("Hook name cannot be empty".to_string()));
        }
        
        if hook_name.len() > 100 {
            return Err(AppError::ValidationError(format!(
                "Hook name too long: {} characters (max 100)",
                hook_name.len()
            )));
        }
        
        // Check for valid characters (alphanumeric, underscore, hyphen)
        if !hook_name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Err(AppError::ValidationError(format!(
                "Hook name contains invalid characters: {}",
                hook_name
            )));
        }
        
        Ok(())
    }
}

impl Default for DefaultHookValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl HookValidator for DefaultHookValidator {
    fn validate_input(&self, hook_name: &str, data: &Value) -> AppResult<()> {
        // Validate hook name
        self.validate_hook_name(hook_name)
            .with_context("Hook name validation failed")?;
        
        // Validate JSON structure and security
        self.validate_json_structure(data, 0)
            .with_context("JSON structure validation failed")?;
        
        // Validate required fields
        self.validate_required_fields(hook_name, data)
            .with_context("Required fields validation failed")?;
        
        Ok(())
    }
    
    fn validate_processed(&self, hook: &ProcessedHook) -> AppResult<()> {
        // Validate the hook name again
        self.validate_hook_name(&hook.hook_name)
            .with_context("Processed hook name validation failed")?;
        
        // Validate enhanced data structure
        self.validate_json_structure(&hook.enhanced_data, 0)
            .with_context("Enhanced data validation failed")?;
        
        // Ensure timestamp is reasonable (not too far in future or past)
        let now = chrono::Utc::now();
        let time_diff = (now - hook.timestamp).num_seconds().abs();
        if time_diff > 3600 { // More than 1 hour difference
            return Err(AppError::ValidationError(format!(
                "Hook timestamp is too far from current time: {} seconds",
                time_diff
            )));
        }
        
        // Validate that original and enhanced data are both valid JSON
        if !hook.original_data.is_object() && !hook.original_data.is_array() && !hook.original_data.is_null() {
            // Allow primitive types for original data
        }
        
        if !hook.enhanced_data.is_object() && !hook.enhanced_data.is_array() && !hook.enhanced_data.is_null() {
            // Allow primitive types for enhanced data
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use chrono::Utc;
    use crate::hooks::types::{HookMetadata, SystemInfo, ClaudeEnvironment};
    
    #[test]
    fn test_input_validation_success() {
        let validator = DefaultHookValidator::new();
        let data = json!({"test": "data", "number": 42});
        
        let result = validator.validate_input("PostToolUse", &data);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_forbidden_field_detection() {
        let validator = DefaultHookValidator::new();
        let data = json!({"password": "secret123", "test": "data"});
        
        let result = validator.validate_input("PostToolUse", &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("forbidden"));
    }
    
    #[test]
    fn test_sql_injection_detection() {
        let validator = DefaultHookValidator::new();
        let data = json!({"query": "'; DROP TABLE users; --"});
        
        let result = validator.validate_input("PostToolUse", &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("SQL injection"));
    }
    
    #[test]
    fn test_script_injection_detection() {
        let validator = DefaultHookValidator::new();
        let data = json!({"html": "<script>alert('xss')</script>"});
        
        let result = validator.validate_input("PostToolUse", &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("script injection"));
    }
    
    #[test]
    fn test_required_fields_validation() {
        let validator = DefaultHookValidator::new();
        let data = json!({"other": "field"});
        
        let result = validator.validate_input("PreTask", &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Required field 'task_id'"));
    }
    
    #[test]
    fn test_processed_hook_validation() {
        let validator = DefaultHookValidator::new();
        
        let metadata = HookMetadata {
            git_info: None,
            user_info: None,
            system_info: SystemInfo::current(),
            environment: std::collections::HashMap::new(),
            claude_env: ClaudeEnvironment::from_env(),
        };
        
        let hook = ProcessedHook::new(
            "PostToolUse".to_string(),
            json!({"test": "data"}),
            json!({"test": "data", "success": true}),
            metadata,
        );
        
        let result = validator.validate_processed(&hook);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_invalid_hook_name() {
        let validator = DefaultHookValidator::new();
        let data = json!({"test": "data"});
        
        let result = validator.validate_input("Invalid Hook Name!", &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid characters"));
    }
    
    #[test]
    fn test_max_depth_validation() {
        let validator = DefaultHookValidator::with_config(
            2, // max_depth = 2
            1000,
            HashSet::new(),
        );
        
        let data = json!({
            "level1": {
                "level2": {
                    "level3": {
                        "level4": "too deep"
                    }
                }
            }
        });
        
        let result = validator.validate_input("PostToolUse", &data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("maximum depth"));
    }
}