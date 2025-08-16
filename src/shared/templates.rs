use anyhow::{Context, Result};
use chrono::Local;
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Template style configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateStyle {
    /// Rich formatting with emojis and detailed information (for CLI)
    Rich,
    /// Compact formatting with minimal emojis (for daemon)
    Compact,
}

impl Default for TemplateStyle {
    fn default() -> Self {
        TemplateStyle::Rich
    }
}

#[derive(Debug, Clone)]
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
    default_templates: HashMap<String, String>,
}

impl TemplateEngine {

    pub fn new_with_style(_style: TemplateStyle) -> Result<Self> {
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
        Self::create_rich_templates(&mut templates);
        templates
    }

    fn create_rich_templates(templates: &mut HashMap<String, String>) {
        // PreToolUse hook - Rich style
        templates.insert(
            "PreToolUse".to_string(),
            r#"▶️ **{{ tool_name }}** starting

{{#if tool_input.file_path}}📁 `{{tool_input.file_path}}`{{/if}}
{{#if tool_input.command}}💻 `{{tool_input.command}}`{{/if}}
{{#if tool_input.pattern}}🔍 `{{tool_input.pattern}}`{{/if}}
{{#if tool_input.description}}📝 {{tool_input.description}}{{/if}}
{{#if cwd}}📂 {{cwd}}{{/if}}

{{timestamp}}"#
                .to_string(),
        );

        // PostToolUse hook - Rich style
        templates.insert(
            "PostToolUse".to_string(),
            r#"{{#if tool_response.error}}❌ **{{ tool_name }}** failed{{else}}✅ **{{ tool_name }}** completed{{/if}}

{{#if tool_response.error}}Error: {{tool_response.error}}{{/if}}
{{#if tool_response.filePath}}📁 `{{tool_response.filePath}}`{{/if}}
{{#if tool_response.content}}{{#if (gt (len tool_response.content) 100)}}📄 Output: {{len tool_response.content}} chars{{else}}📄 `{{tool_response.content}}`{{/if}}{{/if}}
{{#if duration_ms}}⏱️ {{duration_ms}}ms{{/if}}
{{#if tool_response.exit_code}}🔢 Exit: {{tool_response.exit_code}}{{/if}}

{{timestamp}}"#
                .to_string(),
        );

        // Add other rich templates...
        Self::add_common_rich_templates(templates);
    }


    fn add_common_rich_templates(templates: &mut HashMap<String, String>) {
        // UserPromptSubmit hook
        templates.insert(
            "UserPromptSubmit".to_string(),
            r#"💬 **User prompt**

{{prompt}}
{{#if cwd}}📂 {{cwd}}{{/if}}

{{timestamp}}"#
                .to_string(),
        );

        // SessionStart hook
        templates.insert(
            "SessionStart".to_string(),
            r#"🚀 **Session started**

{{#if cwd}}📂 {{cwd}}{{/if}}
{{#if session_id}}🔗 {{session_id}}{{/if}}

{{timestamp}}"#
                .to_string(),
        );

        // Stop hook
        templates.insert(
            "Stop".to_string(),
            r#"🛑 **Session ended**

{{#if session_duration}}⏱️ {{session_duration}}{{/if}}
{{#if final_status}}📊 {{final_status}}{{/if}}

{{timestamp}}"#
                .to_string(),
        );

        // Generic hook template
        templates.insert(
            "generic".to_string(),
            r#"🔔 **{{hook_name}}**

{{#if message}}{{message}}{{/if}}

{{timestamp}}"#
                .to_string(),
        );
    }


    pub fn render(&self, template_name: &str, data: &Value) -> Result<String> {
        // Add timestamp to data
        let mut context = data.clone();
        if let Value::Object(ref mut map) = context {
            map.insert("timestamp".to_string(), Value::String(Local::now().format("%H:%M:%S").to_string()));
        }

        self.handlebars
            .render(template_name, &context)
            .context(format!("Failed to render template: {template_name}"))
    }


    // Format hook data for compatibility with old API
    pub fn format_hook_data(&self, _hook_name: &str, hook_data: &Value) -> Value {
        // Add timestamp to hook data
        let mut formatted = hook_data.clone();
        if let Value::Object(ref mut map) = formatted {
            map.insert("timestamp".to_string(), Value::String(Local::now().format("%H:%M:%S").to_string()));
        }
        formatted
    }


    pub fn get_template(&self, name: &str) -> Option<String> {
        self.default_templates.get(name).cloned()
    }

    pub fn get_template_list(&self) -> Vec<String> {
        self.default_templates.keys().cloned().collect()
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageFormatter {
    pub title_template: Option<String>,
    pub body_template: Option<String>,
    pub priority_map: HashMap<String, u8>,
    pub tag_map: HashMap<String, Vec<String>>,
}

impl MessageFormatter {
    pub fn new(style: TemplateStyle) -> Self {
        match style {
            TemplateStyle::Rich => Self::rich_formatter(),
            TemplateStyle::Compact => Self::compact_formatter(),
        }
    }

    fn rich_formatter() -> Self {
        let mut priority_map = HashMap::new();
        // Rich style - balanced priorities
        priority_map.insert("SessionStart".to_string(), 2);
        priority_map.insert("Stop".to_string(), 2);
        priority_map.insert("PreToolUse".to_string(), 2);
        priority_map.insert("PostToolUse".to_string(), 3);
        priority_map.insert("UserPromptSubmit".to_string(), 4);
        priority_map.insert("Notification".to_string(), 4);
        priority_map.insert("SubagentStop".to_string(), 2);

        let mut tag_map = HashMap::new();
        // Rich style - clean tags
        tag_map.insert(
            "PreToolUse".to_string(),
            vec!["tool".to_string(), "start".to_string()],
        );
        tag_map.insert(
            "PostToolUse".to_string(),
            vec!["tool".to_string(), "done".to_string()],
        );
        tag_map.insert(
            "UserPromptSubmit".to_string(),
            vec!["prompt".to_string(), "user".to_string()],
        );
        tag_map.insert(
            "SessionStart".to_string(),
            vec!["session".to_string(), "start".to_string()],
        );
        tag_map.insert(
            "Stop".to_string(),
            vec!["session".to_string(), "end".to_string()],
        );

        Self {
            title_template: None,
            body_template: None,
            priority_map,
            tag_map,
        }
    }

    fn compact_formatter() -> Self {
        let mut priority_map = HashMap::new();
        // Compact style - unified priorities
        priority_map.insert("SessionStart".to_string(), 2);
        priority_map.insert("Stop".to_string(), 2);
        priority_map.insert("PreToolUse".to_string(), 2);
        priority_map.insert("PostToolUse".to_string(), 3);
        priority_map.insert("UserPromptSubmit".to_string(), 4);
        priority_map.insert("Notification".to_string(), 4);
        priority_map.insert("SubagentStop".to_string(), 2);

        let mut tag_map = HashMap::new();
        // Compact style - minimal tags
        tag_map.insert(
            "PreToolUse".to_string(),
            vec!["tool".to_string()],
        );
        tag_map.insert(
            "PostToolUse".to_string(),
            vec!["tool".to_string()],
        );
        tag_map.insert(
            "UserPromptSubmit".to_string(),
            vec!["prompt".to_string()],
        );
        tag_map.insert(
            "SessionStart".to_string(),
            vec!["session".to_string()],
        );
        tag_map.insert(
            "Stop".to_string(),
            vec!["session".to_string()],
        );

        Self {
            title_template: None,
            body_template: None,
            priority_map,
            tag_map,
        }
    }


    pub fn get_tags(&self, hook_name: &str) -> Vec<String> {
        self.tag_map.get(hook_name).cloned().unwrap_or_default()
    }

    // Format title for notification messages
    pub fn format_title(&self, hook_name: &str, _data: &Value) -> String {
        match hook_name {
            "PreToolUse" => "Tool Starting".to_string(),
            "PostToolUse" => "Tool Completed".to_string(),
            "UserPromptSubmit" => "User Prompt".to_string(),
            "SessionStart" => "Session Started".to_string(),
            "Stop" => "Session Ended".to_string(),
            "SubagentStop" => "Agent Finished".to_string(),
            _ => hook_name.to_string(),
        }
    }
}

impl Default for MessageFormatter {
    fn default() -> Self {
        Self::new(TemplateStyle::default())
    }
}