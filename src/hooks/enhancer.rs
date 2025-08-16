//! Hook data enhancement logic
//! 
//! This module contains the logic for enhancing hook data, including
//! inferring missing fields and transforming data for better processing.

use serde_json::{Value, json};
use crate::errors::{AppResult, ErrorContextExt};

/// Trait for hook data enhancers
/// 
/// Enhancers transform and enrich hook data, such as inferring success
/// states for PostToolUse hooks or adding computed fields.
pub trait HookDataEnhancer: Send + Sync {
    /// Enhance hook data for the given hook type
    /// 
    /// # Arguments
    /// * `hook_name` - The name of the hook being enhanced
    /// * `data` - The original hook data
    /// 
    /// # Returns
    /// Enhanced hook data with additional or modified fields
    fn enhance(&self, hook_name: &str, data: Value) -> AppResult<Value>;
}

/// Default implementation of HookDataEnhancer
/// 
/// This enhancer implements the critical logic extracted from handlers.rs,
/// particularly the PostToolUse success field inference.
pub struct DefaultHookDataEnhancer {
    // Configuration options for enhancement
    infer_success_field: bool,
    add_timestamps: bool,
}

impl DefaultHookDataEnhancer {
    /// Create a new DefaultHookDataEnhancer with default settings
    pub fn new() -> Self {
        Self {
            infer_success_field: true,
            add_timestamps: true,
        }
    }
    
    
    /// Enhance PostToolUse hook data
    /// 
    /// This is the critical enhancement logic from handlers.rs:607-655
    /// that infers the success field from tool_response data.
    fn enhance_post_tool_use(&self, mut data: Value) -> AppResult<Value> {
        if !self.infer_success_field {
            return Ok(data);
        }
        
        // Check if success field is already present
        if data.get("success").is_some() {
            return Ok(data);
        }
        
        // Try to infer success from tool_response
        let success = if let Some(tool_response) = data.get("tool_response") {
            self.infer_success_from_tool_response(tool_response)
        } else {
            // If no tool_response, try to infer from other fields
            self.infer_success_from_context(&data)
        };
        
        // Add the inferred success field
        if let Some(obj) = data.as_object_mut() {
            obj.insert("success".to_string(), json!(success));
        }
        
        Ok(data)
    }
    
    /// Infer success from tool_response data
    /// 
    /// This implements the complex logic from the original enhance_hook_data function
    fn infer_success_from_tool_response(&self, tool_response: &Value) -> bool {
        // Check for explicit error indicators
        if let Some(error) = tool_response.get("error") {
            if !error.is_null() {
                return false;
            }
        }
        
        // Check for status field
        if let Some(status) = tool_response.get("status") {
            if let Some(status_str) = status.as_str() {
                return match status_str.to_lowercase().as_str() {
                    "success" | "ok" | "completed" => true,
                    "error" | "failed" | "failure" => false,
                    _ => true, // Default to success for unknown status
                };
            }
        }
        
        // Check for exit_code (for command tools)
        if let Some(exit_code) = tool_response.get("exit_code") {
            if let Some(code) = exit_code.as_i64() {
                return code == 0;
            }
        }
        
        // Check for success field in tool_response
        if let Some(success) = tool_response.get("success") {
            if let Some(success_bool) = success.as_bool() {
                return success_bool;
            }
        }
        
        // Check for output content (presence usually indicates success)
        if let Some(output) = tool_response.get("output") {
            if !output.is_null() && output.as_str().is_none_or(|s| !s.trim().is_empty()) {
                return true;
            }
        }
        
        // Default to success if no clear indicators
        true
    }
    
    /// Infer success from other context when tool_response is missing
    fn infer_success_from_context(&self, data: &Value) -> bool {
        // Check for direct error field
        if let Some(error) = data.get("error") {
            if !error.is_null() {
                return false;
            }
        }
        
        // Check for exception field
        if let Some(exception) = data.get("exception") {
            if !exception.is_null() {
                return false;
            }
        }
        
        // Default to success
        true
    }
    
    /// Add timestamp fields to hook data
    fn add_timestamp_fields(&self, mut data: Value) -> AppResult<Value> {
        if !self.add_timestamps {
            return Ok(data);
        }
        
        let now = chrono::Utc::now();
        
        if let Some(obj) = data.as_object_mut() {
            // Only add if not already present
            if !obj.contains_key("processed_at") {
                obj.insert("processed_at".to_string(), json!(now.to_rfc3339()));
            }
            
            if !obj.contains_key("timestamp") {
                obj.insert("timestamp".to_string(), json!(now.timestamp()));
            }
        }
        
        Ok(data)
    }
}

impl Default for DefaultHookDataEnhancer {
    fn default() -> Self {
        Self::new()
    }
}

impl HookDataEnhancer for DefaultHookDataEnhancer {
    fn enhance(&self, hook_name: &str, data: Value) -> AppResult<Value> {
        let mut enhanced_data = data;
        
        // Apply hook-specific enhancements
        enhanced_data = match hook_name {
            "PostToolUse" => self.enhance_post_tool_use(enhanced_data)
                .with_context("Failed to enhance PostToolUse hook data")?,
            _ => enhanced_data,
        };
        
        // Apply common enhancements
        enhanced_data = self.add_timestamp_fields(enhanced_data)
            .with_context("Failed to add timestamp fields")?;
        
        Ok(enhanced_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_post_tool_use_success_inference() {
        let enhancer = DefaultHookDataEnhancer::new();
        
        // Test with explicit success in tool_response
        let data = json!({
            "tool_response": {
                "success": true,
                "output": "Command completed"
            }
        });
        
        let result = enhancer.enhance("PostToolUse", data).unwrap();
        assert_eq!(result.get("success").unwrap().as_bool().unwrap(), true);
    }
    
    #[test]
    fn test_post_tool_use_error_inference() {
        let enhancer = DefaultHookDataEnhancer::new();
        
        // Test with error in tool_response
        let data = json!({
            "tool_response": {
                "error": "Command failed",
                "exit_code": 1
            }
        });
        
        let result = enhancer.enhance("PostToolUse", data).unwrap();
        assert_eq!(result.get("success").unwrap().as_bool().unwrap(), false);
    }
    
    #[test]
    fn test_post_tool_use_exit_code_inference() {
        let enhancer = DefaultHookDataEnhancer::new();
        
        // Test with exit_code = 0 (success)
        let data = json!({
            "tool_response": {
                "exit_code": 0,
                "output": "Success"
            }
        });
        
        let result = enhancer.enhance("PostToolUse", data).unwrap();
        assert_eq!(result.get("success").unwrap().as_bool().unwrap(), true);
        
        // Test with exit_code != 0 (failure)
        let data = json!({
            "tool_response": {
                "exit_code": 1,
                "output": "Error occurred"
            }
        });
        
        let result = enhancer.enhance("PostToolUse", data).unwrap();
        assert_eq!(result.get("success").unwrap().as_bool().unwrap(), false);
    }
    
    #[test]
    fn test_timestamp_addition() {
        let enhancer = DefaultHookDataEnhancer::new();
        let data = json!({"test": "data"});
        
        let result = enhancer.enhance("TestHook", data).unwrap();
        
        assert!(result.get("processed_at").is_some());
        assert!(result.get("timestamp").is_some());
    }
    
    #[test]
    fn test_existing_success_preserved() {
        let enhancer = DefaultHookDataEnhancer::new();
        
        // Test that existing success field is preserved
        let data = json!({
            "success": false,
            "tool_response": {
                "success": true
            }
        });
        
        let result = enhancer.enhance("PostToolUse", data).unwrap();
        assert_eq!(result.get("success").unwrap().as_bool().unwrap(), false);
    }
}