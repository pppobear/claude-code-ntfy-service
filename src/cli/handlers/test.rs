//! Test notification handler
//!
//! This module handles test notification commands for verifying
//! the notification system configuration.

use super::super::CliContext;
use crate::shared::clients::create_async_client_from_ntfy_config;
use anyhow::Result;

/// Handler for test operations
pub struct TestHandler<'a> {
    context: &'a CliContext,
}

impl<'a> TestHandler<'a> {
    /// Create new test handler
    pub fn new(context: &'a CliContext) -> Self {
        Self { context }
    }

    /// Handle test notification
    pub async fn handle_test(
        &self,
        message: String,
        title: Option<String>,
        priority: u8,
        topic: Option<String>,
    ) -> Result<()> {
        let config_manager = &self.context.config_manager;
        let config = config_manager.config();

        let client = create_async_client_from_ntfy_config(&config.ntfy)?;

        let topic = topic.unwrap_or_else(|| config.ntfy.default_topic.clone());
        let title = title.unwrap_or_else(|| "Claude Ntfy Test".to_string());

        client.send_simple(&topic, &title, &message, priority).await?;

        println!("Test notification sent successfully");
        println!("Topic: {topic}");
        println!("Title: {title}");
        println!("Message: {message}");
        println!("Priority: {priority}");

        Ok(())
    }
}

// Implement the handler factory trait to reduce boilerplate
super::traits::impl_context_handler!(TestHandler<'a>);
