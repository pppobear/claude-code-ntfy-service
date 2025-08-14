//! Type definitions for hooks processing
//! 
//! This module contains all the data structures used throughout the hooks
//! processing pipeline, including processed hooks, metadata, and supporting types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A processed hook with enhanced data and metadata
/// 
/// This structure represents a hook after it has been processed through
/// the enhancement and validation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedHook {
    /// The name of the hook (e.g., "PostToolUse", "PreTask")
    pub hook_name: String,
    
    /// When the hook was processed
    pub timestamp: DateTime<Utc>,
    
    /// The original hook data as received
    pub original_data: Value,
    
    /// Enhanced hook data with inferred fields and processed values
    pub enhanced_data: Value,
    
    /// Collected metadata about the hook context
    pub metadata: HookMetadata,
}

/// Metadata collected during hook processing
/// 
/// Contains contextual information about the environment, git state,
/// user info, and system details when the hook was triggered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMetadata {
    /// Git repository information
    pub git_info: Option<GitInfo>,
    
    /// User information
    pub user_info: Option<UserInfo>,
    
    /// System information
    pub system_info: SystemInfo,
    
    /// Environment variables at hook processing time
    pub environment: HashMap<String, String>,
    
    /// Claude-specific environment variables
    pub claude_env: ClaudeEnvironment,
}

/// Git repository information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    /// Current branch name
    pub branch: Option<String>,
    
    /// Current commit hash
    pub commit: Option<String>,
    
    /// Repository root directory
    pub repo_root: Option<String>,
    
    /// Whether there are uncommitted changes
    pub has_changes: bool,
    
    /// Remote origin URL (if available)
    pub remote_url: Option<String>,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// Username
    pub username: Option<String>,
    
    /// User's full name (from git config)
    pub full_name: Option<String>,
    
    /// User's email (from git config)
    pub email: Option<String>,
}

/// System information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// Operating system name
    pub os: String,
    
    /// System architecture
    pub arch: String,
    
    /// Hostname
    pub hostname: Option<String>,
    
    /// Current working directory
    pub cwd: Option<String>,
    
    /// Process ID
    pub pid: u32,
}

/// Claude-specific environment variables
/// 
/// Extracted from CLAUDE_* environment variables set by Claude Code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeEnvironment {
    /// Tool name from CLAUDE_TOOL_NAME
    pub tool_name: Option<String>,
    
    /// Tool status from CLAUDE_TOOL_STATUS
    pub tool_status: Option<String>,
    
    /// Tool duration from CLAUDE_TOOL_DURATION
    pub tool_duration: Option<String>,
    
    /// Project directory from CLAUDE_PROJECT_DIR
    pub project_dir: Option<String>,
    
    /// Workspace from CLAUDE_WORKSPACE
    pub workspace: Option<String>,
    
    /// Tool from CLAUDE_TOOL
    pub tool: Option<String>,
}

/// Hook processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// Whether to enhance hook data
    pub enable_enhancement: bool,
    
    /// Whether to validate hook data
    pub enable_validation: bool,
    
    /// Whether to collect git metadata
    pub collect_git_info: bool,
    
    /// Whether to collect system metadata
    pub collect_system_info: bool,
    
    /// List of hooks to process (empty = all hooks)
    pub allowed_hooks: Vec<String>,
    
    /// List of hooks to ignore
    pub ignored_hooks: Vec<String>,
    
    /// Maximum size of hook data in bytes
    pub max_data_size: Option<usize>,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            enable_enhancement: true,
            enable_validation: true,
            collect_git_info: true,
            collect_system_info: true,
            allowed_hooks: vec![],
            ignored_hooks: vec![],
            max_data_size: Some(1024 * 1024), // 1MB default limit
        }
    }
}

impl ProcessedHook {
    /// Create a new ProcessedHook with current timestamp
    pub fn new(
        hook_name: String,
        original_data: Value,
        enhanced_data: Value,
        metadata: HookMetadata,
    ) -> Self {
        Self {
            hook_name,
            timestamp: Utc::now(),
            original_data,
            enhanced_data,
            metadata,
        }
    }
    
    /// Get a field from the enhanced data
    pub fn get_enhanced_field(&self, field: &str) -> Option<&Value> {
        self.enhanced_data.get(field)
    }
    
    /// Check if this hook was successful (for PostToolUse hooks)
    pub fn is_successful(&self) -> Option<bool> {
        if self.hook_name == "PostToolUse" {
            self.get_enhanced_field("success")
                .and_then(|v| v.as_bool())
        } else {
            None
        }
    }
}

impl SystemInfo {
    /// Create SystemInfo with current system details
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            hostname: hostname::get().ok().and_then(|h| h.into_string().ok()),
            cwd: std::env::current_dir().ok().and_then(|p| p.to_string_lossy().into_owned().into()),
            pid: std::process::id(),
        }
    }
}

impl ClaudeEnvironment {
    /// Extract Claude environment from environment variables
    pub fn from_env() -> Self {
        Self {
            tool_name: std::env::var("CLAUDE_TOOL_NAME").ok(),
            tool_status: std::env::var("CLAUDE_TOOL_STATUS").ok(),
            tool_duration: std::env::var("CLAUDE_TOOL_DURATION").ok(),
            project_dir: std::env::var("CLAUDE_PROJECT_DIR").ok(),
            workspace: std::env::var("CLAUDE_WORKSPACE").ok(),
            tool: std::env::var("CLAUDE_TOOL").ok(),
        }
    }
}