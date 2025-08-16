use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration structure for the Claude Code Ntfy Service
///
/// Contains all configuration sections including ntfy server settings,
/// hook processing configuration, template settings, and daemon options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ntfy: NtfyConfig,
    pub hooks: HookConfig,
    pub templates: TemplateConfig,
    pub daemon: DaemonConfig,
}

/// Configuration for ntfy notification service integration
///
/// Contains settings for connecting to and sending notifications via ntfy servers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NtfyConfig {
    pub server_url: String,
    pub default_topic: String,
    pub default_priority: Option<u8>,
    pub default_tags: Option<Vec<String>>,
    pub auth_token: Option<String>,
    pub timeout_secs: Option<u64>,
    pub send_format: String, // "text" or "json"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub enabled: bool,
    pub topics: HashMap<String, String>, // hook_name -> topic
    pub priorities: HashMap<String, u8>, // hook_name -> priority
    pub filters: HashMap<String, Vec<String>>, // hook_name -> filters
    #[serde(default = "default_never_filter_decision_hooks")]
    pub never_filter_decision_hooks: bool, // Always allow decision-requiring hooks
    #[serde(default = "default_decision_hook_priority")]
    pub decision_hook_priority: u8, // Priority for hooks that require user decisions
}

fn default_never_filter_decision_hooks() -> bool {
    true
}

fn default_decision_hook_priority() -> u8 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    pub use_custom: bool,
    pub custom_templates: HashMap<String, String>, // hook_name -> template
    pub variables: HashMap<String, String>,        // custom variables
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub enabled: bool,
    pub socket_path: Option<PathBuf>,
    pub log_level: String,
    pub log_path: Option<String>,
    pub max_queue_size: usize,
    pub retry_attempts: u32,
    pub retry_delay_secs: u64,
}

impl Config {
    /// Default hook topics for different hook types
    pub fn default_hook_topics() -> HashMap<String, String> {
        let mut topics = HashMap::new();
        topics.insert("Notification".to_string(), "claude-decisions".to_string());
        topics.insert("PreToolUse".to_string(), "claude-tools".to_string());
        topics.insert("UserPromptSubmit".to_string(), "claude-prompts".to_string());
        topics
    }
    
    /// Default hook priorities - higher values = higher priority (1-5 scale)
    pub fn default_hook_priorities() -> HashMap<String, u8> {
        let mut priorities = HashMap::new();
        
        // Decision-requiring hooks get max priority (5)
        priorities.insert("Notification".to_string(), 5);
        priorities.insert("PreToolUse".to_string(), 4); // May require decisions
        priorities.insert("UserPromptSubmit".to_string(), 4); // May block prompts
        
        // Other hooks get lower priority
        priorities.insert("PostToolUse".to_string(), 3);
        priorities.insert("SessionStart".to_string(), 2);
        priorities.insert("Stop".to_string(), 2);
        priorities.insert("SubagentStop".to_string(), 2);
        priorities.insert("PreCompact".to_string(), 1);
        
        priorities
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ntfy: NtfyConfig {
                server_url: "https://ntfy.sh".to_string(),
                default_topic: "claude-code-hooks".to_string(),
                default_priority: Some(3),
                default_tags: Some(vec!["claude-code".to_string()]),
                auth_token: None,
                timeout_secs: Some(30),
                send_format: "text".to_string(), // Default to text for better compatibility
            },
            hooks: HookConfig {
                enabled: true,
                topics: Self::default_hook_topics(),
                priorities: Self::default_hook_priorities(),
                filters: HashMap::new(),
                never_filter_decision_hooks: true,
                decision_hook_priority: 5, // Max priority for decision hooks
            },
            templates: TemplateConfig {
                use_custom: false,
                custom_templates: HashMap::new(),
                variables: HashMap::new(),
            },
            daemon: DaemonConfig {
                enabled: true,
                socket_path: None,
                log_level: "info".to_string(),
                log_path: None, // Default to None, will use console logging
                max_queue_size: 1000,
                retry_attempts: 3,
                retry_delay_secs: 5,
            },
        }
    }
}

