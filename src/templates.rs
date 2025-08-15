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
            r#"{{#if (eq tool_name "Read")}}📖{{else if (eq tool_name "Write")}}✍️{{else if (eq tool_name "Edit")}}📝{{else if (eq tool_name "Bash")}}💻{{else if (eq tool_name "Grep")}}🔍{{else if (eq tool_name "Glob")}}📁{{else if (eq tool_name "Task")}}🤖{{else}}🔧{{/if}} **Starting {{ tool_name }}**

{{#if tool_input.file_path}}📄 **File:** `{{tool_input.file_path}}`{{/if}}
{{#if tool_input.command}}⚡ **Command:** `{{tool_input.command}}`{{/if}}
{{#if tool_input.pattern}}🔍 **Pattern:** `{{tool_input.pattern}}`{{/if}}
{{#if tool_input.description}}📋 **Description:** {{tool_input.description}}{{/if}}
{{#if cwd}}📂 **Directory:** `{{cwd}}`{{/if}}

⏰ {{timestamp}}"#
                .to_string(),
        );

        // PostToolUse hook
        templates.insert(
            "PostToolUse".to_string(),
            r#"{{#if tool_response.error}}❌ **FAILED:**{{else}}✅ **COMPLETED:**{{/if}} **{{ tool_name }}**

{{#if tool_response.error}}🚨 **Error Details:**
```
{{tool_response.error}}
```{{else}}✨ **Status:** Success{{/if}}

{{#if tool_response.filePath}}📄 **File:** `{{tool_response.filePath}}`{{/if}}
{{#if tool_response.content}}📊 **Output:** {{#if (gt (len tool_response.content) 100)}}*Large output ({{len tool_response.content}} chars)*{{else}}`{{tool_response.content}}`{{/if}}{{/if}}
{{#if duration_ms}}⏱️ **Duration:** {{duration_ms}}ms{{/if}}
{{#if tool_response.exit_code}}🔢 **Exit Code:** {{tool_response.exit_code}}{{/if}}

⏰ {{timestamp}}"#
                .to_string(),
        );

        // UserPromptSubmit hook
        templates.insert(
            "UserPromptSubmit".to_string(),
            r#"💬 **New User Message**

📝 **Prompt:**
> {{#if (gt (len prompt) 200)}}{{substr prompt 0 200}}...{{else}}{{prompt}}{{/if}}

{{#if session_id}}🔗 **Session:** `{{session_id}}`{{/if}}
{{#if cwd}}📂 **Working Dir:** `{{cwd}}`{{/if}}
{{#if project_name}}📁 **Project:** {{project_name}}{{/if}}

⏰ {{timestamp}}"#
                .to_string(),
        );

        // SessionStart hook
        templates.insert(
            "SessionStart".to_string(),
            r#"🚀 **Claude Code Session Started**

{{#if session_id}}🔗 **Session ID:** `{{session_id}}`{{/if}}
{{#if cwd}}📂 **Working Directory:** `{{cwd}}`{{/if}}
{{#if source}}📍 **Source:** {{source}}{{/if}}
{{#if git_branch}}🌿 **Git Branch:** `{{git_branch}}`{{/if}}
{{#if project_name}}📁 **Project:** {{project_name}}{{/if}}
{{#if user}}👤 **User:** {{user}}{{/if}}

✨ Ready for AI-powered development assistance!

⏰ {{timestamp}}"#
                .to_string(),
        );

        // Stop hook
        templates.insert(
            "Stop".to_string(),
            r#"🏁 **Claude Code Session Ended**

{{#if session_id}}🔗 **Session ID:** `{{session_id}}`{{/if}}
{{#if session_duration}}⏱️ **Duration:** {{session_duration}}{{/if}}
{{#if tools_used}}🔧 **Tools Used:** {{tools_used}}{{/if}}
{{#if files_modified}}📝 **Files Modified:** {{files_modified}}{{/if}}
{{#if error_count}}⚠️ **Errors:** {{error_count}}{{/if}}
{{#if stop_hook_active}}🔌 **Stop Hook Active:** {{stop_hook_active}}{{/if}}

📊 Session completed successfully

⏰ {{timestamp}}"#
                .to_string(),
        );

        // Notification hook
        templates.insert(
            "Notification".to_string(),
            r#"🔔 **System Notification**

{{#if message}}📢 **Message:** {{message}}{{/if}}
{{#if notification_type}}📋 **Type:** {{notification_type}}{{/if}}
{{#if session_id}}🔗 **Session:** `{{session_id}}`{{/if}}
{{#if priority_level}}⚡ **Priority:** {{priority_level}}{{/if}}

⏰ {{timestamp}}"#
                .to_string(),
        );

        // SubagentStop hook
        templates.insert(
            "SubagentStop".to_string(),
            r#"🤖 **Subagent Stopped**

{{#if agent_name}}🏷️ **Agent:** {{agent_name}}{{/if}}
{{#if session_id}}🔗 **Session:** `{{session_id}}`{{/if}}
{{#if agent_duration}}⏱️ **Runtime:** {{agent_duration}}{{/if}}
{{#if tasks_completed}}✅ **Tasks Completed:** {{tasks_completed}}{{/if}}
{{#if stop_hook_active}}🔌 **Stop Hook Active:** {{stop_hook_active}}{{/if}}

🔚 Agent execution finished

⏰ {{timestamp}}"#
                .to_string(),
        );

        // Generic hook template for any unrecognized hooks
        templates.insert(
            "generic".to_string(),
            r#"🔧 **{{hook_name}} Hook**

{{#each data}}{{#unless (eq @key "timestamp")}}📋 **{{@key}}:** {{#if (eq (typeof this) "string")}}{{#if (gt (len this) 100)}}*{{len this}} characters*{{else}}`{{this}}`{{/if}}{{else}}{{this}}{{/if}}
{{/unless}}{{/each}}

🔔 Custom hook notification

⏰ {{timestamp}}"#
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
        // Official Claude Code hook priorities (1=min, 2=low, 3=default, 4=high, 5=max/urgent)
        priority_map.insert("SessionStart".to_string(), 3); // Important session events
        priority_map.insert("Stop".to_string(), 3); // Important session events
        priority_map.insert("PreToolUse".to_string(), 2); // Lower priority for starting actions
        priority_map.insert("PostToolUse".to_string(), 3); // Default for completed actions (errors handled dynamically)
        priority_map.insert("UserPromptSubmit".to_string(), 4); // High priority for user interactions
        priority_map.insert("Notification".to_string(), 4); // High priority for system notifications
        priority_map.insert("SubagentStop".to_string(), 2); // Lower priority for agent completion

        let mut tag_map = HashMap::new();
        // Official Claude Code hook tags with emoji-compatible names
        tag_map.insert(
            "PreToolUse".to_string(),
            vec!["wrench".to_string(), "arrow_forward".to_string(), "tools".to_string()],
        );
        tag_map.insert(
            "PostToolUse".to_string(),
            vec!["white_check_mark".to_string(), "tools".to_string(), "finished".to_string()],
        );
        tag_map.insert(
            "UserPromptSubmit".to_string(),
            vec!["speech_balloon".to_string(), "user".to_string(), "input".to_string()],
        );
        tag_map.insert(
            "SessionStart".to_string(),
            vec!["rocket".to_string(), "new".to_string(), "session".to_string()],
        );
        tag_map.insert(
            "Stop".to_string(),
            vec!["checkered_flag".to_string(), "end".to_string(), "session".to_string()],
        );
        tag_map.insert("Notification".to_string(), vec!["bell".to_string(), "alert".to_string()]);
        tag_map.insert(
            "SubagentStop".to_string(),
            vec!["robot".to_string(), "finished".to_string(), "agent".to_string()],
        );

        MessageFormatter {
            title_template: Some("{{#if (eq hook_name \"PreToolUse\")}}🔧 Starting {{tool_name}}{{else if (eq hook_name \"PostToolUse\")}}{{#if tool_response.error}}❌ {{tool_name}} Failed{{else}}✅ {{tool_name}} Complete{{/if}}{{else if (eq hook_name \"UserPromptSubmit\")}}💬 New User Request{{else if (eq hook_name \"SessionStart\")}}🚀 Claude Session Started{{else if (eq hook_name \"Stop\")}}🏁 Session Ended{{else if (eq hook_name \"Notification\")}}🔔 System Alert{{else if (eq hook_name \"SubagentStop\")}}🤖 Agent Complete{{else}}🔔 {{hook_name}}{{/if}}".to_string()),
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
