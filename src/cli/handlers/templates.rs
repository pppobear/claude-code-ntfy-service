//! Template management handler
//!
//! This module handles template-related commands including
//! listing available templates and displaying their content.

use crate::shared::templates::{TemplateEngine, TemplateStyle};
use anyhow::Result;

/// Handler for template operations
pub struct TemplateHandler;

impl TemplateHandler {
    /// Create new template handler
    pub fn new() -> Self {
        Self
    }

    /// Handle template operations
    pub async fn handle_templates(&self, show: Option<String>) -> Result<()> {
        let template_engine = TemplateEngine::new_with_style(TemplateStyle::Rich)?;

        if let Some(template_name) = show {
            if let Some(content) = template_engine.get_template(&template_name) {
                println!("Template: {template_name}");
                println!("---");
                println!("{content}");
            } else {
                println!("Template '{template_name}' not found");
            }
        } else {
            println!("Available templates:");
            for template in template_engine.get_template_list() {
                println!("  - {template}");
            }
            println!("\nUse 'claude-ntfy templates --show <name>' to view a template");
        }

        Ok(())
    }
}

// Implement the stateless handler factory trait to reduce boilerplate
super::traits::impl_stateless_handler!(TemplateHandler);
