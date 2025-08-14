use anyhow::{Context, Result};
use chrono::Local;
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
    #[allow(dead_code)]
    default_templates: HashMap<String, String>,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let mut handlebars = Handlebars::new();
        handlebars.set_strict_mode(false);

        let default_templates = Self::create_default_templates();

        // Register default templates
        for (name, template) in &default_templates {
            handlebars
                .register_template_string(name, template)
                .context(format!("Failed to register default template: {name}"))?;
        }

        Ok(TemplateEngine {
            handlebars,
            default_templates,
        })
    }

    fn create_default_templates() -> HashMap<String, String> {
        let mut templates = HashMap::new();

        // Official Claude Code hooks

        // PreToolUse hook
        templates.insert(
            "PreToolUse".to_string(),
            r#"🔧 Tool Starting: {{tool_name}}
{{#if tool_input.file_path}}File: {{tool_input.file_path}}{{/if}}
{{#if tool_input.command}}Command: {{tool_input.command}}{{/if}}
Time: {{timestamp}}"#
                .to_string(),
        );

        // PostToolUse hook
        templates.insert(
            "PostToolUse".to_string(),
            r#"{{#if tool_response.error}}❌{{else}}{{#if tool_response.success}}✅{{else}}✅{{/if}}{{/if}} Tool Completed: {{tool_name}}
Status: {{#if tool_response.error}}Failed{{else}}Success{{/if}}
{{#if tool_response.filePath}}File: {{tool_response.filePath}}{{/if}}
{{#if tool_response.error}}Error: {{tool_response.error}}{{/if}}
{{#if duration_ms}}Duration: {{duration_ms}}ms{{/if}}
Time: {{timestamp}}"#
                .to_string(),
        );

        // UserPromptSubmit hook
        templates.insert(
            "UserPromptSubmit".to_string(),
            r#"💬 User Prompt
Message: {{prompt}}
{{#if session_id}}Session: {{session_id}}{{/if}}
Time: {{timestamp}}"#
                .to_string(),
        );

        // SessionStart hook
        templates.insert(
            "SessionStart".to_string(),
            r#"🚀 Claude Code Session Started
{{#if session_id}}Session: {{session_id}}{{/if}}
{{#if cwd}}Working Dir: {{cwd}}{{/if}}
{{#if source}}Source: {{source}}{{/if}}
Time: {{timestamp}}"#
                .to_string(),
        );

        // Stop hook
        templates.insert(
            "Stop".to_string(),
            r#"🏁 Claude Code Session Ended
{{#if session_id}}Session: {{session_id}}{{/if}}
{{#if stop_hook_active}}Stop Hook Active: {{stop_hook_active}}{{/if}}
Time: {{timestamp}}"#
                .to_string(),
        );

        // Notification hook
        templates.insert(
            "Notification".to_string(),
            r#"🔔 Notification
{{#if message}}Message: {{message}}{{/if}}
{{#if session_id}}Session: {{session_id}}{{/if}}
Time: {{timestamp}}"#
                .to_string(),
        );

        // SubagentStop hook
        templates.insert(
            "SubagentStop".to_string(),
            r#"🤖 Subagent Stopped
{{#if session_id}}Session: {{session_id}}{{/if}}
{{#if stop_hook_active}}Stop Hook Active: {{stop_hook_active}}{{/if}}
Time: {{timestamp}}"#
                .to_string(),
        );

        // Generic hook template for any unrecognized hooks
        templates.insert(
            "generic".to_string(),
            r#"🔔 {{hook_name}}
{{#each data}}
{{@key}}: {{this}}
{{/each}}
Time: {{timestamp}}"#
                .to_string(),
        );

        templates
    }

    #[allow(dead_code)]
    pub fn register_custom_template(&mut self, name: &str, template: &str) -> Result<()> {
        self.handlebars
            .register_template_string(name, template)
            .context(format!("Failed to register custom template: {name}"))?;
        Ok(())
    }

    pub fn render(
        &self,
        template_name: &str,
        data: &Value,
        custom_vars: Option<&HashMap<String, String>>,
    ) -> Result<String> {
        // Prepare data with additional context
        let mut context = data.clone();

        // Add timestamp if not present
        if context.get("timestamp").is_none() {
            if let Value::Object(ref mut map) = context {
                map.insert(
                    "timestamp".to_string(),
                    Value::String(Local::now().format("%Y-%m-%d %H:%M:%S").to_string()),
                );
            }
        }

        // Add custom variables if provided
        if let Some(vars) = custom_vars {
            if let Value::Object(ref mut map) = context {
                for (key, value) in vars {
                    map.insert(key.clone(), Value::String(value.clone()));
                }
            }
        }

        // Try to render with the specified template
        let result = if self.handlebars.has_template(template_name) {
            self.handlebars
                .render(template_name, &context)
                .context(format!("Failed to render template: {template_name}"))?
        } else {
            // Fall back to generic template if specified template not found
            self.handlebars
                .render("generic", &context)
                .context("Failed to render generic template")?
        };

        Ok(result)
    }

    pub fn format_hook_data(&self, hook_name: &str, hook_data: &Value) -> Value {
        let mut formatted = hook_data.clone();

        // Add hook name to the data
        if let Value::Object(ref mut map) = formatted {
            map.insert(
                "hook_name".to_string(),
                Value::String(hook_name.to_string()),
            );
        }

        formatted
    }

    #[allow(dead_code)]
    pub fn get_template_list(&self) -> Vec<String> {
        let mut templates: Vec<String> = self.default_templates.keys().cloned().collect();
        templates.sort();
        templates
    }

    #[allow(dead_code)]
    pub fn get_template(&self, name: &str) -> Option<String> {
        self.default_templates.get(name).cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFormatter {
    pub title_template: Option<String>,
    pub body_template: Option<String>,
    pub priority_map: HashMap<String, u8>,
    pub tag_map: HashMap<String, Vec<String>>,
}

impl Default for MessageFormatter {
    fn default() -> Self {
        let mut priority_map = HashMap::new();
        // Official Claude Code hook priorities
        priority_map.insert("SessionStart".to_string(), 2);
        priority_map.insert("Stop".to_string(), 2);
        priority_map.insert("PreToolUse".to_string(), 3);
        priority_map.insert("PostToolUse".to_string(), 3);
        priority_map.insert("UserPromptSubmit".to_string(), 3);
        priority_map.insert("Notification".to_string(), 3);
        priority_map.insert("SubagentStop".to_string(), 2);

        let mut tag_map = HashMap::new();
        // Official Claude Code hook tags
        tag_map.insert(
            "PreToolUse".to_string(),
            vec!["tool".to_string(), "start".to_string()],
        );
        tag_map.insert(
            "PostToolUse".to_string(),
            vec!["tool".to_string(), "complete".to_string()],
        );
        tag_map.insert(
            "UserPromptSubmit".to_string(),
            vec!["user".to_string(), "prompt".to_string()],
        );
        tag_map.insert(
            "SessionStart".to_string(),
            vec!["session".to_string(), "start".to_string()],
        );
        tag_map.insert(
            "Stop".to_string(),
            vec!["session".to_string(), "stop".to_string()],
        );
        tag_map.insert("Notification".to_string(), vec!["notification".to_string()]);
        tag_map.insert(
            "SubagentStop".to_string(),
            vec!["subagent".to_string(), "stop".to_string()],
        );

        MessageFormatter {
            title_template: Some("Claude Code: {{hook_name}}".to_string()),
            body_template: None,
            priority_map,
            tag_map,
        }
    }
}

impl MessageFormatter {
    pub fn format_title(&self, hook_name: &str, data: &Value) -> String {
        if let Some(template) = &self.title_template {
            let mut hb = Handlebars::new();
            hb.set_strict_mode(false);

            let mut context = data.clone();
            if let Value::Object(ref mut map) = context {
                map.insert(
                    "hook_name".to_string(),
                    Value::String(hook_name.to_string()),
                );
            }

            hb.render_template(template, &context)
                .unwrap_or_else(|_| format!("Claude Code: {hook_name}"))
        } else {
            format!("Claude Code: {hook_name}")
        }
    }

    #[allow(dead_code)]
    pub fn get_priority(&self, hook_name: &str) -> u8 {
        self.priority_map.get(hook_name).cloned().unwrap_or(3)
    }

    pub fn get_tags(&self, hook_name: &str) -> Option<Vec<String>> {
        self.tag_map.get(hook_name).cloned()
    }
}
